use super::tree_prefixes;

#[test]
fn guides_follow_the_shape_of_the_tree() {
    // root
    // ├─ a
    // │  └─ b
    // └─ c
    //    └─ d
    assert_eq!(
        tree_prefixes(&[0, 1, 2, 1, 2]),
        vec!["", "├─ ", "│  └─ ", "└─ ", "   └─ "]
    );
}

#[test]
fn a_straight_chain_is_all_last_children() {
    assert_eq!(tree_prefixes(&[0, 1, 2]), vec!["", "└─ ", "   └─ "]);
}

#[test]
fn siblings_under_a_deep_node_keep_the_ancestor_bar() {
    // root
    // ├─ ctx
    // │  ├─ impl
    // │  └─ inv
    // └─ docs
    assert_eq!(
        tree_prefixes(&[0, 1, 2, 2, 1]),
        vec!["", "├─ ", "│  ├─ ", "│  └─ ", "└─ "]
    );
}
