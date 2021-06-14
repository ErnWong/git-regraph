use git2::{Commit, Oid, Reference, Repository, Signature, Tree};

#[derive(Debug)]
pub enum Error {
    Git2Error(git2::Error),
    OriginalMessageNotValidUtf8,
}

impl From<git2::Error> for Error {
    fn from(error: git2::Error) -> Self {
        Error::Git2Error(error)
    }
}

pub enum RefArg<'a> {
    AllLocalRefs,
    Refs(&'a [Reference<'a>]),
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

    fn create_edited_commit(&self, repo: &Repository, original: &Commit) -> Result<Oid, Error> {
        Ok(repo.commit(
            None,
            self.author.unwrap_or(&original.author()),
            self.committer.unwrap_or(&original.committer()),
            self.message.unwrap_or(
                original
                    .message()
                    .ok_or(Error::OriginalMessageNotValidUtf8)?,
            ),
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
    ) -> Result<(), Error>;
}

impl RepositoryExt for Repository {
    fn regraph(
        &self,
        refs_to_update: RefArg,
        commit_to_edit: &Commit,
        edit: &CommitEdit,
    ) -> Result<(), Error> {
        fn update_affected_commits() {
            todo!();
        }

        fn update_refs() {
            todo!();
        }

        edit.create_edited_commit(self, &commit_to_edit)?;
        update_affected_commits();
        update_refs();

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::{Index, IndexAddOption, Oid, Sort, Time};
    use std::{collections::HashMap, fs::File, io::Write};
    use tempfile::{tempdir, TempDir};

    fn given_repository<'a>(
        graph: &[(&'a str, i64, &[&str])],
    ) -> (Repository, HashMap<&'a str, Oid>) {
        fn add_commit<'a, 'b>(
            dir: &TempDir,
            index: &'b mut Index,
            repo: &'a Repository,
            label: &str,
            time_sec: i64,
            parents: &[Commit],
        ) -> Oid {
            File::create(dir.path().join(label)).unwrap();
            let mut shared = File::create(dir.path().join("shared")).unwrap();
            writeln!(shared, "{}", label).unwrap();
            index
                .add_all(&["."], IndexAddOption::DEFAULT, None)
                .unwrap();
            let email = format!("{}-email", label);
            let time = Time::new(time_sec, 0);
            let author = Signature::new(&format!("{}-author", label), &email, &time).unwrap();
            let committer = Signature::new(&format!("{}-comitter", label), &email, &time).unwrap();
            let tree = repo
                .find_tree(index.write_tree().map_err(|e| e.to_string()).unwrap())
                .unwrap();
            let parents_refs: Vec<&Commit> = parents.iter().collect();
            repo.commit(
                Some("HEAD"),
                &author,
                &committer,
                label,
                &tree,
                &parents_refs,
            )
            .unwrap()
        }
        let dir = tempdir().unwrap();
        let repo = Repository::init(&dir).unwrap();
        let mut index = repo.index().unwrap();
        let mut label_to_commit_oid = HashMap::new();
        for (label, time, parents) in graph {
            assert!(
                !label_to_commit_oid.contains_key(label),
                "No duplicate commit labels"
            );

            let mut parent_commits = Vec::new();
            for parent in *parents {
                parent_commits.push(
                    repo.find_commit(*label_to_commit_oid.get(parent).unwrap())
                        .unwrap(),
                );
            }

            let commit_oid = add_commit(&dir, &mut index, &repo, label, *time, &parent_commits);

            label_to_commit_oid.insert(*label, commit_oid);
        }
        (repo, label_to_commit_oid)
    }

    fn label_to_commit_reachable_from_ref<'a>(
        repo: &'a Repository,
        reference: &str,
    ) -> HashMap<String, Commit<'a>> {
        let mut label_to_commit = HashMap::new();

        let mut revwalk = repo.revwalk().unwrap();
        revwalk.push_ref(reference).unwrap();
        revwalk.set_sorting(Sort::TOPOLOGICAL).unwrap();
        for found_commit in revwalk {
            let commit_oid = found_commit.unwrap();
            let commit = repo.find_commit(commit_oid).unwrap();
            let label = commit.message().unwrap().to_string();
            label_to_commit.insert(label, commit.clone());
        }

        label_to_commit
    }

    #[test]
    fn it_can_squash_to_root() {
        // GIVEN a repo...
        let (repo, label_to_commit_oid) = given_repository(&[
            ("A", 0, &[]),         // With main root.
            ("B", 1, &[]),         // With subtree root.
            ("C", 2, &["B"]),      // With more than one commit in subtree.
            ("D", 3, &["A", "C"]), // With subtree merged into main.
            ("E", 4, &["D"]),      // With commit after merge.
        ]);

        // WHEN we squash B-C by removing parents of C.
        repo.regraph(
            RefArg::AllLocalRefs,
            &repo
                .find_commit(*label_to_commit_oid.get("C").unwrap())
                .unwrap(),
            CommitEdit::new().edit_parents(&[]),
        )
        .unwrap();
        let commits = label_to_commit_reachable_from_ref(&repo, "HEAD");

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
            commits.get("D").unwrap().parent_id(0).unwrap(),
            commits.get("A").unwrap().id(),
            "Commit 'D' should still have 'A' as its first parent."
        );

        // THEN
        assert_eq!(
            commits.get("D").unwrap().parent_id(1).unwrap(),
            commits.get("C").unwrap().id(),
            "Commit 'D' should still have 'C' as its second parent."
        );

        // THEN
        assert!(commits.contains_key("E"), "Commit 'E' is updated");

        // THEN all commits should still have the same trees.
        for (label, commit) in commits.iter() {
            assert_eq!(
                commit.tree_id(),
                repo.find_commit(*label_to_commit_oid.get(label as &str).unwrap())
                    .unwrap()
                    .tree_id(),
                "{}'s tree should be untouched",
                label
            );
        }
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
}
