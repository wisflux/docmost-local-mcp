use docmost_local_mcp::docmost_client::{
    CursorListResult, ListResult, normalize_cursor_list_result, normalize_list_result,
};

#[test]
fn returns_array_results_unchanged() {
    assert_eq!(
        normalize_list_result(Some(ListResult::List(vec![1, 2]))),
        vec![1, 2]
    );
}

#[test]
fn extracts_items_arrays_from_wrapped_responses() {
    assert_eq!(
        normalize_list_result(Some(ListResult::Wrapped {
            items: Some(vec![1]),
        })),
        vec![1]
    );
}

#[test]
fn returns_empty_array_for_null_or_empty_item_collections() {
    assert_eq!(normalize_list_result::<i32>(None), Vec::<i32>::new());
    assert_eq!(
        normalize_list_result(Some(ListResult::<i32>::Wrapped { items: None })),
        Vec::<i32>::new()
    );
}

#[test]
fn extracts_items_from_cursor_paginated_responses() {
    assert_eq!(
        normalize_cursor_list_result(CursorListResult {
            items: Some(vec![1, 2, 3]),
        }),
        vec![1, 2, 3]
    );
}

#[test]
fn returns_empty_array_for_missing_cursor_paginated_items() {
    assert_eq!(
        normalize_cursor_list_result::<i32>(CursorListResult { items: None }),
        Vec::<i32>::new()
    );
}
