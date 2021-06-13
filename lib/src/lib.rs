use git2::{Commit, Reference, Signature, Tree};
use std::fs::File;

pub enum RefArg<'a> {
    AllLocalRefs,
    Refs(&'a [Reference<'a>]),
}

pub enum ParentsEdit<'a> {
    KeepParents,
    SetParents(&'a [Commit<'a>]),
    AddParents(&'a [Commit<'a>]),
}

pub enum MessageEdit<'a> {
    KeepMessage,
    SetParagraphs(&'a [&'a str]),
    SetFile(&'a File),
}

pub enum TreeEdit<'a> {
    KeepTree,
    SetTree(&'a Tree<'a>),
}

pub enum SignatureEdit<'a> {
    KeepSignature,
    SetSignature(&'a Signature<'a>),
}

pub struct CommitEdit<'a> {
    parents: ParentsEdit<'a>,
    message: MessageEdit<'a>,
    tree: TreeEdit<'a>,
    author: SignatureEdit<'a>,
    committer: SignatureEdit<'a>,
}

pub fn regraph(refs_to_update: RefArg, commit_to_edit: Commit, edit: CommitEdit) {
    todo!();
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
