use crate::{Cache, Node};

/// Prints a debug representation of the computed layout for a tree of nodes, starting with the given root node.
pub fn print_node<N: Node>(
    node: &N,
    cache: &impl Cache<Node = N>,
    tree: &N::Tree,
    is_root: bool,
    has_sibling: bool,
    lines_string: String,
) where
    N::CacheKey: Copy + std::fmt::Display,
{
    let fork_string = if is_root {
        "│"
    } else if has_sibling {
        "├───┤"
    } else {
        "└───┤"
    };
    println!(
        "{lines}{fork}{id}| {x:#3} {y:#3} {w:#3} {h:#3}│",
        lines = lines_string,
        fork = fork_string,
        id = node.key(),
        x = cache.posx(node),
        y = cache.posy(node),
        w = cache.width(node),
        h = cache.height(node),
    );
    let bar = if is_root {
        ""
    } else if has_sibling {
        "│   "
    } else {
        "    "
    };
    let new_string = lines_string + bar;

    let mut child_iter = node.children(tree).peekable();

    while let Some(child) = child_iter.next() {
        let has_sibling = child_iter.peek().is_some();
        print_node(child, cache, tree, false, has_sibling, new_string.clone());
    }
}
