use git2::{Commit, Reference, Repository, Signature, Tree};
use std::fs::File;

pub enum RefArg<'a> {
    AllLocalRefs,
    Refs(&'a [Reference<'a>]),
}

pub enum ParentsEdit<'a> {
    KeepParents,
    SetParents(&'a [&'a Commit<'a>]),
    AddParents(&'a [&'a Commit<'a>]),
}

impl<'a> Default for ParentsEdit<'a> {
    fn default() -> Self {
        Self::KeepParents
    }
}

pub enum MessageEdit<'a> {
    KeepMessage,
    SetParagraphs(&'a [&'a str]),
    SetFile(&'a File),
}

impl<'a> Default for MessageEdit<'a> {
    fn default() -> Self {
        Self::KeepMessage
    }
}

pub enum TreeEdit<'a> {
    KeepTree,
    SetTree(&'a Tree<'a>),
}

impl<'a> Default for TreeEdit<'a> {
    fn default() -> Self {
        Self::KeepTree
    }
}

pub enum SignatureEdit<'a> {
    KeepSignature,
    SetSignature(&'a Signature<'a>),
}

impl<'a> Default for SignatureEdit<'a> {
    fn default() -> Self {
        Self::KeepSignature
    }
}

#[derive(Default)]
pub struct CommitEdit<'a> {
    parents: ParentsEdit<'a>,
    message: MessageEdit<'a>,
    tree: TreeEdit<'a>,
    author: SignatureEdit<'a>,
    committer: SignatureEdit<'a>,
}

impl<'a> CommitEdit<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_parents(self, parents: &'a [&'a Commit<'a>]) -> Self {
        assert!(
            matches!(self.parents, ParentsEdit::KeepParents),
            "Overwriting previous intent to modify parents"
        );
        Self {
            parents: ParentsEdit::SetParents(parents),
            ..self
        }
    }

    pub fn add_parents(self, parents: &'a [&'a Commit<'a>]) -> Self {
        assert!(
            matches!(self.parents, ParentsEdit::KeepParents),
            "Overwriting previous intent to modify parents"
        );
        Self {
            parents: ParentsEdit::AddParents(parents),
            ..self
        }
    }

    pub fn set_paragraphs(self, paragraphs: &'a [&'a str]) -> Self {
        assert!(
            matches!(self.message, MessageEdit::KeepMessage),
            "Overwriting previous intent to modify message"
        );
        Self {
            message: MessageEdit::SetParagraphs(paragraphs),
            ..self
        }
    }

    pub fn set_file(self, file: &'a File) -> Self {
        assert!(
            matches!(self.message, MessageEdit::KeepMessage),
            "Overwriting previous intent to modify message"
        );
        Self {
            message: MessageEdit::SetFile(file),
            ..self
        }
    }

    pub fn set_tree(self, tree: &'a Tree<'a>) -> Self {
        assert!(
            matches!(self.tree, TreeEdit::KeepTree),
            "Overwriting previous intent to modify tree"
        );
        Self {
            tree: TreeEdit::SetTree(tree),
            ..self
        }
    }

    pub fn set_author(self, author: &'a Signature<'a>) -> Self {
        assert!(
            matches!(self.author, SignatureEdit::KeepSignature),
            "Overwriting previous intent to modify author"
        );
        Self {
            author: SignatureEdit::SetSignature(author),
            ..self
        }
    }

    pub fn set_committer(self, committer: &'a Signature<'a>) -> Self {
        assert!(
            matches!(self.committer, SignatureEdit::KeepSignature),
            "Overwriting previous intent to modify committer"
        );
        Self {
            committer: SignatureEdit::SetSignature(committer),
            ..self
        }
    }
}

pub trait RepositoryExt {
    fn regraph(&self, refs_to_update: RefArg, commit_to_edit: Commit, edit: CommitEdit);
}

impl RepositoryExt for Repository {
    fn regraph(&self, refs_to_update: RefArg, commit_to_edit: Commit, edit: CommitEdit) {
        todo!();
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
            repo.find_commit(*label_to_commit_oid.get("C").unwrap())
                .unwrap(),
            CommitEdit::new().set_parents(&[]),
        );
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
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
