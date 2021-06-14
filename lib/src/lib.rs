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
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
