use clap::{crate_authors, crate_description, crate_name, crate_version, App, ArgGroup};
use git2::{Commit, Repository};
use git_regraph_lib::{CommitEdit, RefArg, RepositoryExt};
use std::fs::read_to_string;

fn main() {
    let matches = App::new(crate_name!())
        .version(crate_version!())
        .author(crate_authors!())
        .about(crate_description!())
        .args_from_usage(
            "--update-all-local-refs   'Update all commits reachable from any non-remote ref, and update the non-remote refs to point to the updated commits.'
             --update-ref [ref] ...    'Update all commits reachable from this ref, and update this ref to point to these updated commits.'
             <COMMIT>                  'A commit-ish revision specifier of the commit you would like to edit'
             --keep-parents            'Leave the parents of the COMMIT unchanged'
             --clear-parents           'Remove all parents of the COMMIT'
             --parent [PARENT]...      'Specify a parent for the COMMIT.'
             --keep-message            'Leave the message of the COMMIT unchanged'
             --message [MESSAGE]...    'Add a paragraph to the COMMIT.'
             --file [FILE]             'Source the commit message from the file FILE and use it to override the message of COMMIT'
             --keep-tree               'Leave the tree of the COMMIT unchanged'
             --tree [TREE]             'Specify an existing tree object id to override the tree of COMMIT'
             --keep-author             'Leave the author of the COMMIT unchanged'
             --keep-committer          'Leave the commiter of the COMMIT unchanged'
             "
        )
        .group(ArgGroup::with_name("refs")
            .args(&["update-all-local-refs", "update-ref"]).required(true))
        .group(ArgGroup::with_name("parents")
            .args(&["keep-parents", "clear-parents", "parent"]).required(true))
        .group(ArgGroup::with_name("messages")
            .args(&["keep-message", "message", "file"]).required(true))
        .group(ArgGroup::with_name("trees")
            .args(&["keep-tree", "tree"]).required(true))

        // TODO: modify author/committer
        .group(ArgGroup::with_name("author")
            .args(&["keep-author"]).required(true))
        .group(ArgGroup::with_name("committer")
            .args(&["keep-committer"]).required(true))

        .get_matches();

    // TODO: Proper error handling.

    let repo = Repository::open(std::env::current_dir().unwrap()).unwrap();

    let refs_to_update = match (
        matches.is_present("update-all-local-refs"),
        matches.values_of("update-ref"),
    ) {
        (true, None) => RefArg::AllLocalRefs,
        (false, Some(values)) => RefArg::Refs(
            values
                .map(|name| repo.find_reference(name).unwrap())
                .collect(),
        ),
        _ => unreachable!(),
    };

    let commit_to_edit = repo
        .revparse_single(matches.value_of("COMMIT").unwrap())
        .unwrap()
        .into_commit()
        .expect("Specified COMMIT is not a commit");

    let mut edit = CommitEdit::new();

    let parents_edit = matches.values_of("parent").map(|parents| {
        parents
            .map(|revspec| {
                repo.revparse_single(revspec)
                    .unwrap()
                    .into_commit()
                    .expect("Specified PARENT is not a commit")
            })
            .collect::<Vec<Commit>>()
    });
    let parent_refs;
    if let Some(parents) = &parents_edit {
        parent_refs = parents.iter().collect::<Vec<&Commit>>();
        edit.edit_parents(&parent_refs);
    }

    let message_edit = matches
        .values_of("message")
        .map(|paragraphs| paragraphs.collect::<Vec<&str>>().join("\n\n"))
        .or(matches
            .value_of("file")
            .map(|file| read_to_string(file).unwrap()));
    if let Some(message) = &message_edit {
        edit.edit_message(message);
    }

    let tree_edit = matches.value_of("tree").map(|tree_spec| {
        repo.revparse_single(tree_spec)
            .unwrap()
            .into_tree()
            .expect("Specifed TREE is not a tree")
    });
    if let Some(tree) = &tree_edit {
        edit.edit_tree(tree);
    }

    repo.regraph(refs_to_update, &commit_to_edit, &edit)
        .unwrap();
}
