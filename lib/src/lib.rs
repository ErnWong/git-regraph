#![feature(backtrace)]

use git2::{Commit, Oid, Reference, Repository, Signature, Sort, Tree};
use std::{backtrace::Backtrace, collections::HashMap};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum RegraphError {
    #[error("Failed to run git command")]
    Git2Error {
        #[from]
        source: git2::Error,
        backtrace: Backtrace,
    },
    #[error("Commit {commit} does not have a valid utf-8 message and could not be re-applied.")]
    CommitWithInvalidUtf8Message { commit: Oid, backtrace: Backtrace },
    #[error("The specified edit specification does not actually change the commit.")]
    NoChange,
}

pub enum RefArg<'a> {
    AllLocalRefs,
    Refs(Vec<Reference<'a>>),
}

impl<'a> RefArg<'a> {
    pub fn resolve(self, repo: &'a Repository) -> Result<Vec<Reference>, RegraphError> {
        Ok(match self {
            RefArg::AllLocalRefs => repo.references()?.collect::<Result<_, _>>()?,
            RefArg::Refs(refs) => refs,
        })
    }
}

#[derive(Default)]
pub struct CommitEdit<'a> {
    parents: Option<&'a [&'a Commit<'a>]>,
    message: Option<&'a str>,
    tree: Option<&'a Tree<'a>>,
    author: Option<&'a Signature<'a>>,
    committer: Option<&'a Signature<'a>>,
}

impl<'a> CommitEdit<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn edit_parents<'s>(&'s mut self, parents: &'a [&'a Commit<'a>]) -> &'s mut Self {
        assert!(
            self.parents.is_none(),
            "Overwriting previous intent to modify parents"
        );
        self.parents = Some(parents);
        self
    }

    pub fn edit_message<'s>(&'s mut self, message: &'a str) -> &'s mut Self {
        assert!(
            self.message.is_none(),
            "Overwriting previous intent to modify message"
        );
        self.message = Some(message);
        self
    }

    pub fn edit_tree<'s>(&'s mut self, tree: &'a Tree<'a>) -> &'s mut Self {
        assert!(
            self.tree.is_none(),
            "Overwriting previous intent to modify tree"
        );
        self.tree = Some(tree);
        self
    }

    pub fn edit_author<'s>(&'s mut self, author: &'a Signature<'a>) -> &'s mut Self {
        assert!(
            self.author.is_none(),
            "Overwriting previous intent to modify author"
        );
        self.author = Some(author);
        self
    }

    pub fn edit_committer<'s>(&'s mut self, committer: &'a Signature<'a>) -> &'s mut Self {
        assert!(
            self.committer.is_none(),
            "Overwriting previous intent to modify committer"
        );
        self.committer = Some(committer);
        self
    }

    fn create_edited_commit(
        &self,
        repo: &Repository,
        original: &Commit,
    ) -> Result<Oid, RegraphError> {
        Ok(repo.commit(
            None,
            self.author.unwrap_or(&original.author()),
            self.committer.unwrap_or(&original.committer()),
            self.message.unwrap_or(original.message().ok_or(
                RegraphError::CommitWithInvalidUtf8Message {
                    commit: original.id(),
                    backtrace: Backtrace::capture(),
                },
            )?),
            self.tree.unwrap_or(&original.tree()?),
            self.parents.unwrap_or(
                &original
                    .parents()
                    .collect::<Vec<Commit>>()
                    .iter()
                    .collect::<Vec<&Commit>>(),
            ),
        )?)
    }
}

pub trait RepositoryExt {
    fn regraph(
        &self,
        refs_to_update: RefArg,
        commit_to_edit: &Commit,
        edit: &CommitEdit,
    ) -> Result<(), RegraphError>;
}

impl RepositoryExt for Repository {
    fn regraph(
        &self,
        refs_to_update: RefArg,
        commit_to_edit: &Commit,
        edit: &CommitEdit,
    ) -> Result<(), RegraphError> {
        fn discover_old_commits(
            repo: &Repository,
            resolved_refs_to_update: &[Reference],
            edited_commit_oid: Oid,
        ) -> Result<Vec<Oid>, RegraphError> {
            let mut revwalk = repo.revwalk()?;
            revwalk.set_sorting(Sort::TOPOLOGICAL)?;

            for reference in resolved_refs_to_update.iter() {
                revwalk.push(
                    reference
                        .resolve()?
                        .target()
                        .expect("Resolved reference should have a direct target"),
                )?;
            }
            revwalk.hide(edited_commit_oid)?;

            // TODO: We collect into a new vector rather than iterating them in-place, because I'm
            // not sure if editing the git graph while iterating through the RevWalk will
            // invalidate the iterator. This could potentially be better optimised.
            Ok(revwalk.collect::<Result<_, _>>()?)
        }

        fn update_affected_commits(
            repo: &Repository,
            old_commit_oids: &[Oid],
            old_to_new_oids: &mut HashMap<Oid, Oid>,
        ) -> Result<(), RegraphError> {
            for old_oid in old_commit_oids {
                let commit = repo.find_commit(*old_oid)?;

                let needs_updating = commit
                    .parent_ids()
                    .any(|oid| old_to_new_oids.contains_key(&oid));

                if needs_updating {
                    let parents: Vec<Commit> = commit
                        .parent_ids()
                        .map(|oid| *old_to_new_oids.get(&oid).unwrap_or(&oid))
                        .map(|oid| repo.find_commit(oid.clone()))
                        .collect::<Result<_, _>>()?;
                    let parents_ref: Vec<&Commit> = parents.iter().collect();

                    let new_oid = repo.commit(
                        None,
                        &commit.author(),
                        &commit.committer(),
                        commit
                            .message()
                            .ok_or(RegraphError::CommitWithInvalidUtf8Message {
                                commit: commit.id(),
                                backtrace: Backtrace::capture(),
                            })?,
                        &commit.tree()?,
                        &parents_ref,
                    )?;

                    old_to_new_oids.insert(*old_oid, new_oid);
                }
            }
            Ok(())
        }

        fn update_refs(
            resolved_refs_to_update: &[Reference],
            old_edited_oid: &Oid,
            new_edited_oid: &Oid,
            old_to_new_oids: &HashMap<Oid, Oid>,
        ) -> Result<(), RegraphError> {
            let reflog_message = format!(
                "regraph: update after editing commit {} -> {}",
                old_edited_oid, new_edited_oid
            );
            for reference in resolved_refs_to_update {
                let mut direct_ref = reference.resolve()?;
                let old_oid = reference
                    .target()
                    .expect("Direct references should have a direct target");
                if let Some(new_oid) = old_to_new_oids.get(&old_oid) {
                    direct_ref.set_target(*new_oid, &reflog_message)?;
                }
            }
            Ok(())
        }

        let mut old_to_new_oids = HashMap::new();

        let edited_commit_oid = edit.create_edited_commit(self, &commit_to_edit)?;

        if edited_commit_oid == commit_to_edit.id() {
            return Err(RegraphError::NoChange);
        }

        old_to_new_oids.insert(commit_to_edit.id(), edited_commit_oid);

        let resolved_refs_to_update = refs_to_update.resolve(self)?;

        let old_commit_oids =
            discover_old_commits(self, &resolved_refs_to_update, edited_commit_oid)?;

        update_affected_commits(self, &old_commit_oids, &mut old_to_new_oids)?;

        update_refs(
            &resolved_refs_to_update,
            &commit_to_edit.id(),
            &edited_commit_oid,
            &old_to_new_oids,
        )?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use git2::{Index, IndexAddOption, Oid, Sort, Time};
    use std::{collections::HashMap, fs::File, io::Write};
    use tempfile::{tempdir, TempDir};

    fn given_repository<'a>(
        graph: &[(&'a str, i64, &[&str])],
        branches: &[(&str, &str)],
    ) -> Result<(Repository, HashMap<&'a str, Oid>, TempDir)> {
        fn add_commit<'a, 'b>(
            dir: &TempDir,
            index: &'b mut Index,
            repo: &'a Repository,
            label: &str,
            time_sec: i64,
            parents: &[Commit],
        ) -> Result<Oid> {
            File::create(dir.path().join(label))?;
            let mut shared = File::create(dir.path().join("shared"))?;
            writeln!(shared, "{}", label)?;
            index.add_all(&["."], IndexAddOption::DEFAULT, None)?;
            let email = format!("{}-email", label);
            let time = Time::new(time_sec, 0);
            let author = Signature::new(&format!("{}-author", label), &email, &time)?;
            let committer = Signature::new(&format!("{}-comitter", label), &email, &time)?;
            let tree = repo.find_tree(index.write_tree()?)?;
            let parents_refs: Vec<&Commit> = parents.iter().collect();
            Ok(repo.commit(None, &author, &committer, label, &tree, &parents_refs)?)
        }
        let dir = tempdir()?;
        let repo = Repository::init(&dir)?;
        let mut index = repo.index()?;
        let mut label_to_commit_oid = HashMap::new();
        for (label, time, parents) in graph {
            assert!(
                !label_to_commit_oid.contains_key(label),
                "No duplicate commit labels"
            );

            let mut parent_commits = Vec::new();
            for parent in *parents {
                parent_commits.push(repo.find_commit(*label_to_commit_oid.get(parent).unwrap())?);
            }

            let commit_oid = add_commit(&dir, &mut index, &repo, label, *time, &parent_commits)?;

            label_to_commit_oid.insert(*label, commit_oid);
        }
        for (branch_name, target) in branches {
            let commit = repo.find_commit(*label_to_commit_oid.get(target).unwrap())?;
            repo.branch(branch_name, &commit, true)?;
        }
        Ok((repo, label_to_commit_oid, dir))
    }

    fn label_to_commit_reachable_from_ref<'a>(
        repo: &'a Repository,
        reference: &str,
    ) -> Result<HashMap<String, Commit<'a>>> {
        let mut label_to_commit = HashMap::new();

        let mut revwalk = repo.revwalk()?;
        revwalk.push_ref(reference)?;
        revwalk.set_sorting(Sort::TOPOLOGICAL)?;
        for found_commit in revwalk {
            let commit_oid = found_commit?;
            let commit = repo.find_commit(commit_oid)?;
            let label = commit.message().unwrap().to_string();
            label_to_commit.insert(label, commit.clone());
        }

        Ok(label_to_commit)
    }

    #[test]
    fn it_can_squash_to_root() -> Result<()> {
        // GIVEN a repo...
        let (repo, label_to_commit_oid, _dir) = given_repository(
            &[
                ("A", 0, &[]),         // With main root.
                ("B", 1, &[]),         // With subtree root.
                ("C", 2, &["B"]),      // With more than one commit in subtree.
                ("D", 3, &["A", "C"]), // With subtree merged into main.
                ("E", 4, &["D"]),      // With commit after merge.
            ],
            &[("master", "E")],
        )?;

        // WHEN we squash B-C by removing parents of C.
        repo.regraph(
            RefArg::AllLocalRefs,
            &repo.find_commit(*label_to_commit_oid.get("C").unwrap())?,
            CommitEdit::new().edit_parents(&[]),
        )?;
        let commits = label_to_commit_reachable_from_ref(&repo, "HEAD")?;

        // THEN
        assert_eq!(
            commits.get("A").unwrap().id(),
            *label_to_commit_oid.get("A").unwrap(),
            "Commit 'A' should remain unaffected, since it doesn't depend on 'C'"
        );

        // THEN
        assert!(
            !commits.contains_key("B"),
            "Commit 'B' should no longer be in the graph."
        );

        // THEN
        assert_eq!(
            commits.get("C").unwrap().parent_count(),
            0,
            "Commit 'C' should have no parent."
        );

        // THEN
        assert_eq!(
            commits.get("D").unwrap().parent_count(),
            2,
            "Commit 'D' should still have 2 parents."
        );

        // THEN
        assert_eq!(
            commits.get("D").unwrap().parent_id(0)?,
            commits.get("A").unwrap().id(),
            "Commit 'D' should still have 'A' as its first parent."
        );

        // THEN
        assert_eq!(
            commits.get("D").unwrap().parent_id(1)?,
            commits.get("C").unwrap().id(),
            "Commit 'D' should still have 'C' as its second parent."
        );

        // THEN
        assert!(commits.contains_key("E"), "Commit 'E' is updated");

        // THEN all commits should still have the same trees.
        for (label, commit) in commits.iter() {
            assert_eq!(
                commit.tree_id(),
                repo.find_commit(*label_to_commit_oid.get(label as &str).unwrap())?
                    .tree_id(),
                "{}'s tree should be untouched",
                label
            );
        }

        Ok(())
    }

    #[test]
    #[ignore]
    fn it_update_notes() {
        todo!();
    }

    #[test]
    #[ignore]
    fn it_can_leave_some_refs_untouched() {
        todo!();
    }

    #[test]
    #[ignore]
    fn it_can_squash_some_commits() {
        todo!();
    }

    #[test]
    #[ignore]
    fn it_can_change_authors() {
        todo!();
    }

    #[test]
    #[ignore]
    fn it_can_change_committers() {
        todo!();
    }

    #[test]
    #[ignore]
    fn it_can_swap_parents() {
        todo!();
    }

    #[test]
    #[ignore]
    fn it_can_unsquash() {
        todo!();
    }

    #[test]
    #[ignore]
    fn it_can_append_parents() {
        todo!();
    }

    #[test]
    #[ignore]
    fn it_can_swap_trees() {
        todo!();
    }

    #[test]
    #[ignore]
    fn it_leaves_remote_refs_untouched() {
        todo!();
    }

    #[test]
    #[ignore]
    fn it_errors_when_edit_does_no_change() {
        todo!();
    }
}
