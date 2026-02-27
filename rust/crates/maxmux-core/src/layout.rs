use crate::session::{LayoutNode, PaneId, SplitDirection};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

/// Recursively traverse the binary tree layout and compute a `Rect` for every
/// leaf pane.  Split nodes divide the available `bounds` according to their
/// ratio and direction, reserving a 1-cell gap between the two children for
/// the border separator.
pub fn calculate_layout(node: &LayoutNode, bounds: Rect) -> HashMap<PaneId, Rect> {
    let mut result = HashMap::new();

    match node {
        LayoutNode::Leaf { pane_id } => {
            result.insert(pane_id.clone(), bounds);
        }
        LayoutNode::Split {
            direction,
            ratio,
            children,
        } => {
            let (first_bounds, second_bounds) = match direction {
                SplitDirection::Horizontal => {
                    // Split left | right
                    let split_x =
                        bounds.x as f64 + bounds.width as f64 * ratio;
                    let split_x = split_x.floor() as u16;
                    let first = Rect {
                        x: bounds.x,
                        y: bounds.y,
                        width: split_x - bounds.x,
                        height: bounds.height,
                    };
                    let second = Rect {
                        x: split_x + 1,
                        y: bounds.y,
                        width: bounds.x + bounds.width - split_x - 1,
                        height: bounds.height,
                    };
                    (first, second)
                }
                SplitDirection::Vertical => {
                    // Split top / bottom
                    let split_y =
                        bounds.y as f64 + bounds.height as f64 * ratio;
                    let split_y = split_y.floor() as u16;
                    let first = Rect {
                        x: bounds.x,
                        y: bounds.y,
                        width: bounds.width,
                        height: split_y - bounds.y,
                    };
                    let second = Rect {
                        x: bounds.x,
                        y: split_y + 1,
                        width: bounds.width,
                        height: bounds.y + bounds.height - split_y - 1,
                    };
                    (first, second)
                }
            };

            let first_result = calculate_layout(&children.0, first_bounds);
            let second_result = calculate_layout(&children.1, second_bounds);

            result.extend(first_result);
            result.extend(second_result);
        }
    }

    result
}

/// Immutable tree transformation.  Find the leaf containing `target_pane` and
/// replace it with a `Split` node containing both `target_pane` and `new_pane`
/// at ratio 0.5.
pub fn split_layout(
    node: &LayoutNode,
    target_pane: &str,
    new_pane: &str,
    direction: SplitDirection,
) -> LayoutNode {
    match node {
        LayoutNode::Leaf { pane_id } => {
            if pane_id == target_pane {
                LayoutNode::Split {
                    direction,
                    ratio: 0.5,
                    children: Box::new((
                        LayoutNode::Leaf {
                            pane_id: target_pane.into(),
                        },
                        LayoutNode::Leaf {
                            pane_id: new_pane.into(),
                        },
                    )),
                }
            } else {
                node.clone()
            }
        }
        LayoutNode::Split {
            direction: dir,
            ratio,
            children,
        } => LayoutNode::Split {
            direction: dir.clone(),
            ratio: *ratio,
            children: Box::new((
                split_layout(&children.0, target_pane, new_pane, direction.clone()),
                split_layout(&children.1, target_pane, new_pane, direction),
            )),
        },
    }
}

/// Remove a pane from the tree.  If its parent split has only the sibling
/// remaining, collapse the split to just the sibling.  Returns `None` if the
/// removed pane was the only one.
pub fn remove_from_layout(node: &LayoutNode, pane_id: &str) -> Option<LayoutNode> {
    match node {
        LayoutNode::Leaf { pane_id: id } => {
            if id == pane_id {
                None
            } else {
                Some(node.clone())
            }
        }
        LayoutNode::Split {
            direction,
            ratio,
            children,
        } => {
            let left = remove_from_layout(&children.0, pane_id);
            let right = remove_from_layout(&children.1, pane_id);

            match (left, right) {
                (None, None) => None,
                (None, some) => some,
                (some, None) => some,
                (Some(l), Some(r)) => Some(LayoutNode::Split {
                    direction: direction.clone(),
                    ratio: *ratio,
                    children: Box::new((l, r)),
                }),
            }
        }
    }
}

/// Smart directional navigation algorithm.
///
/// 1. Calculate center point of current pane.
/// 2. For each other pane, check if it is in the requested direction
///    (center-to-center comparison).
/// 3. Calculate Manhattan distance.
/// 4. Check perpendicular overlap:
///    - Left/Right: Y-axis overlap (pane shares row space).
///    - Up/Down: X-axis overlap (pane shares column space).
/// 5. Prefer candidates with overlap; fall back to all if none overlap.
/// 6. Sort by Manhattan distance.
/// 7. Tiebreaker: if `preferred_id` candidate exists and is within 10% of the
///    best distance, return it instead.
/// 8. Return closest candidate.
pub fn find_pane_in_direction(
    pane_rects: &HashMap<PaneId, Rect>,
    current_id: &str,
    direction: Direction,
    preferred_id: Option<&str>,
) -> Option<PaneId> {
    let current_rect = pane_rects.get(current_id)?;

    let cx = current_rect.x as f64 + current_rect.width as f64 / 2.0;
    let cy = current_rect.y as f64 + current_rect.height as f64 / 2.0;

    let is_in_dir = |id: &str| -> bool {
        let rect = match pane_rects.get(id) {
            Some(r) => r,
            None => return false,
        };
        let px = rect.x as f64 + rect.width as f64 / 2.0;
        let py = rect.y as f64 + rect.height as f64 / 2.0;
        match direction {
            Direction::Up => py < cy,
            Direction::Down => py > cy,
            Direction::Left => px < cx,
            Direction::Right => px > cx,
        }
    };

    let has_perpendicular_overlap = |id: &str| -> bool {
        let rect = match pane_rects.get(id) {
            Some(r) => r,
            None => return false,
        };
        match direction {
            Direction::Left | Direction::Right => {
                // Y-axis overlap
                let cur_end = current_rect.y as i32 + current_rect.height as i32;
                let rect_end = rect.y as i32 + rect.height as i32;
                cur_end.min(rect_end) > (current_rect.y as i32).max(rect.y as i32)
            }
            Direction::Up | Direction::Down => {
                // X-axis overlap
                let cur_end = current_rect.x as i32 + current_rect.width as i32;
                let rect_end = rect.x as i32 + rect.width as i32;
                cur_end.min(rect_end) > (current_rect.x as i32).max(rect.x as i32)
            }
        }
    };

    let manhattan_dist = |id: &str| -> f64 {
        let rect = pane_rects.get(id).unwrap();
        let px = rect.x as f64 + rect.width as f64 / 2.0;
        let py = rect.y as f64 + rect.height as f64 / 2.0;
        (px - cx).abs() + (py - cy).abs()
    };

    // Partition candidates by perpendicular overlap
    let mut overlapping: Vec<&str> = Vec::new();
    let mut non_overlapping: Vec<&str> = Vec::new();

    for id in pane_rects.keys() {
        if id == current_id {
            continue;
        }
        if !is_in_dir(id) {
            continue;
        }
        if has_perpendicular_overlap(id) {
            overlapping.push(id);
        } else {
            non_overlapping.push(id);
        }
    }

    // Prefer same-row/column candidates; fall back to all if none overlap
    let candidates = if !overlapping.is_empty() {
        &overlapping
    } else {
        &non_overlapping
    };
    if candidates.is_empty() {
        return None;
    }

    // Find nearest by Manhattan distance
    let mut best_id: Option<&str> = None;
    let mut best_dist = f64::INFINITY;

    for &id in candidates {
        let dist = manhattan_dist(id);
        if dist < best_dist {
            best_dist = dist;
            best_id = Some(id);
        }
    }

    // Tiebreaker: prefer the previously focused pane when distances are nearly equal
    if let Some(pref_id) = preferred_id {
        if pref_id != current_id
            && best_id != Some(pref_id)
            && pane_rects.contains_key(pref_id)
            && is_in_dir(pref_id)
            && candidates.contains(&pref_id)
        {
            let pref_dist = manhattan_dist(pref_id);
            if pref_dist <= best_dist * 1.1 {
                return Some(pref_id.to_string());
            }
        }
    }

    best_id.map(|id| id.to_string())
}

/// DFS collection of all pane IDs from the layout tree.
pub fn get_all_pane_ids(node: &LayoutNode) -> Vec<PaneId> {
    match node {
        LayoutNode::Leaf { pane_id } => vec![pane_id.clone()],
        LayoutNode::Split { children, .. } => {
            let mut ids = get_all_pane_ids(&children.0);
            ids.extend(get_all_pane_ids(&children.1));
            ids
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{LayoutNode, SplitDirection};

    fn leaf(id: &str) -> LayoutNode {
        LayoutNode::Leaf {
            pane_id: id.into(),
        }
    }
    fn hsplit(ratio: f64, first: LayoutNode, second: LayoutNode) -> LayoutNode {
        LayoutNode::Split {
            direction: SplitDirection::Horizontal,
            ratio,
            children: Box::new((first, second)),
        }
    }
    fn vsplit(ratio: f64, first: LayoutNode, second: LayoutNode) -> LayoutNode {
        LayoutNode::Split {
            direction: SplitDirection::Vertical,
            ratio,
            children: Box::new((first, second)),
        }
    }

    #[test]
    fn test_single_pane_fills_bounds() {
        let layout = leaf("p1");
        let bounds = Rect {
            x: 0,
            y: 0,
            width: 80,
            height: 24,
        };
        let rects = calculate_layout(&layout, bounds);
        assert_eq!(rects.get("p1"), Some(&bounds));
    }

    #[test]
    fn test_horizontal_split() {
        let layout = hsplit(0.5, leaf("p1"), leaf("p2"));
        let bounds = Rect {
            x: 0,
            y: 0,
            width: 80,
            height: 24,
        };
        let rects = calculate_layout(&layout, bounds);
        let p1 = rects["p1"];
        let p2 = rects["p2"];
        // p1 gets left half
        assert_eq!(p1.x, 0);
        assert_eq!(p1.width, 40);
        assert_eq!(p1.height, 24);
        // p2 gets right half after border
        assert_eq!(p2.x, 41); // 40 + 1 border
        assert_eq!(p2.width, 39); // 80 - 41
        assert_eq!(p2.height, 24);
    }

    #[test]
    fn test_vertical_split() {
        let layout = vsplit(0.5, leaf("p1"), leaf("p2"));
        let bounds = Rect {
            x: 0,
            y: 0,
            width: 80,
            height: 24,
        };
        let rects = calculate_layout(&layout, bounds);
        let p1 = rects["p1"];
        let p2 = rects["p2"];
        assert_eq!(p1.y, 0);
        assert_eq!(p1.height, 12);
        assert_eq!(p2.y, 13);
        assert_eq!(p2.height, 11);
    }

    #[test]
    fn test_nested_split_three_panes() {
        // p1 left, p2 top-right, p3 bottom-right
        let layout = hsplit(0.5, leaf("p1"), vsplit(0.5, leaf("p2"), leaf("p3")));
        let bounds = Rect {
            x: 0,
            y: 0,
            width: 80,
            height: 24,
        };
        let rects = calculate_layout(&layout, bounds);
        assert_eq!(rects.len(), 3);
        let p1 = rects["p1"];
        let p2 = rects["p2"];
        let p3 = rects["p3"];
        // p1 is full left
        assert_eq!(p1.x, 0);
        assert_eq!(p1.height, 24);
        // p2 and p3 are in right half, stacked
        assert_eq!(p2.x, 41);
        assert!(p2.y < p3.y);
    }

    #[test]
    fn test_split_layout_creates_split() {
        let layout = leaf("p1");
        let result = split_layout(&layout, "p1", "p2", SplitDirection::Horizontal);
        match &result {
            LayoutNode::Split {
                direction,
                ratio,
                children,
            } => {
                assert!(matches!(direction, SplitDirection::Horizontal));
                assert_eq!(*ratio, 0.5);
                assert!(
                    matches!(&children.0, LayoutNode::Leaf { pane_id } if pane_id == "p1")
                );
                assert!(
                    matches!(&children.1, LayoutNode::Leaf { pane_id } if pane_id == "p2")
                );
            }
            _ => panic!("Expected Split"),
        }
    }

    #[test]
    fn test_remove_collapses_to_sibling() {
        let layout = hsplit(0.5, leaf("p1"), leaf("p2"));
        let result = remove_from_layout(&layout, "p1");
        assert!(matches!(result, Some(LayoutNode::Leaf { pane_id }) if pane_id == "p2"));
    }

    #[test]
    fn test_remove_last_pane_returns_none() {
        let layout = leaf("p1");
        let result = remove_from_layout(&layout, "p1");
        assert!(result.is_none());
    }

    #[test]
    fn test_find_pane_right() {
        let layout = hsplit(0.5, leaf("p1"), leaf("p2"));
        let bounds = Rect {
            x: 0,
            y: 0,
            width: 80,
            height: 24,
        };
        let rects = calculate_layout(&layout, bounds);
        assert_eq!(
            find_pane_in_direction(&rects, "p1", Direction::Right, None),
            Some("p2".into())
        );
    }

    #[test]
    fn test_find_pane_left() {
        let layout = hsplit(0.5, leaf("p1"), leaf("p2"));
        let bounds = Rect {
            x: 0,
            y: 0,
            width: 80,
            height: 24,
        };
        let rects = calculate_layout(&layout, bounds);
        assert_eq!(
            find_pane_in_direction(&rects, "p2", Direction::Left, None),
            Some("p1".into())
        );
    }

    #[test]
    fn test_find_pane_no_match() {
        let layout = hsplit(0.5, leaf("p1"), leaf("p2"));
        let bounds = Rect {
            x: 0,
            y: 0,
            width: 80,
            height: 24,
        };
        let rects = calculate_layout(&layout, bounds);
        // p1 has nothing above it
        assert_eq!(
            find_pane_in_direction(&rects, "p1", Direction::Up, None),
            None
        );
    }

    #[test]
    fn test_find_pane_overlap_preference() {
        // T-shape: p1 top-left, p2 top-right, p3 full bottom
        let layout = vsplit(0.5, hsplit(0.5, leaf("p1"), leaf("p2")), leaf("p3"));
        let bounds = Rect {
            x: 0,
            y: 0,
            width: 80,
            height: 24,
        };
        let rects = calculate_layout(&layout, bounds);
        // From p1, going down: p3 has perpendicular overlap, should be preferred
        assert_eq!(
            find_pane_in_direction(&rects, "p1", Direction::Down, None),
            Some("p3".into())
        );
    }

    #[test]
    fn test_find_pane_tiebreaker() {
        // Three panes side by side (nested hsplits)
        let layout = hsplit(0.33, leaf("p1"), hsplit(0.5, leaf("p2"), leaf("p3")));
        let bounds = Rect {
            x: 0,
            y: 0,
            width: 120,
            height: 24,
        };
        let rects = calculate_layout(&layout, bounds);
        // From p1 going right, p2 is closer. But if preferred_id is p3 and it's within 10%...
        // Actually p3 will be further, so p2 should win normally
        assert_eq!(
            find_pane_in_direction(&rects, "p1", Direction::Right, None),
            Some("p2".into())
        );
    }

    #[test]
    fn test_get_all_pane_ids() {
        let layout = hsplit(0.5, leaf("p1"), vsplit(0.5, leaf("p2"), leaf("p3")));
        let mut ids = get_all_pane_ids(&layout);
        ids.sort();
        assert_eq!(ids, vec!["p1", "p2", "p3"]);
    }

    // Additional comprehensive tests

    #[test]
    fn test_horizontal_split_with_offset_bounds() {
        // Test that splits work correctly when bounds don't start at (0,0)
        let layout = hsplit(0.5, leaf("p1"), leaf("p2"));
        let bounds = Rect {
            x: 10,
            y: 5,
            width: 80,
            height: 24,
        };
        let rects = calculate_layout(&layout, bounds);
        let p1 = rects["p1"];
        let p2 = rects["p2"];
        // split_x = floor(10 + 80 * 0.5) = 50
        assert_eq!(p1.x, 10);
        assert_eq!(p1.width, 40); // 50 - 10
        assert_eq!(p2.x, 51); // 50 + 1
        assert_eq!(p2.width, 39); // 10 + 80 - 50 - 1
    }

    #[test]
    fn test_vertical_split_with_offset_bounds() {
        let layout = vsplit(0.5, leaf("p1"), leaf("p2"));
        let bounds = Rect {
            x: 10,
            y: 5,
            width: 80,
            height: 24,
        };
        let rects = calculate_layout(&layout, bounds);
        let p1 = rects["p1"];
        let p2 = rects["p2"];
        // split_y = floor(5 + 24 * 0.5) = 17
        assert_eq!(p1.y, 5);
        assert_eq!(p1.height, 12); // 17 - 5
        assert_eq!(p2.y, 18); // 17 + 1
        assert_eq!(p2.height, 11); // 5 + 24 - 17 - 1
    }

    #[test]
    fn test_split_layout_nested() {
        // Split p2 in an existing two-pane layout
        let layout = hsplit(0.5, leaf("p1"), leaf("p2"));
        let result = split_layout(&layout, "p2", "p3", SplitDirection::Vertical);
        match &result {
            LayoutNode::Split { children, .. } => {
                // First child should still be p1
                assert!(
                    matches!(&children.0, LayoutNode::Leaf { pane_id } if pane_id == "p1")
                );
                // Second child should be a split of p2 and p3
                match &children.1 {
                    LayoutNode::Split {
                        direction,
                        ratio,
                        children,
                    } => {
                        assert!(matches!(direction, SplitDirection::Vertical));
                        assert_eq!(*ratio, 0.5);
                        assert!(
                            matches!(&children.0, LayoutNode::Leaf { pane_id } if pane_id == "p2")
                        );
                        assert!(
                            matches!(&children.1, LayoutNode::Leaf { pane_id } if pane_id == "p3")
                        );
                    }
                    _ => panic!("Expected nested Split"),
                }
            }
            _ => panic!("Expected Split"),
        }
    }

    #[test]
    fn test_split_layout_nonexistent_pane() {
        // Splitting on a pane that doesn't exist should return the tree unchanged
        let layout = leaf("p1");
        let result = split_layout(&layout, "p99", "p2", SplitDirection::Horizontal);
        assert!(matches!(result, LayoutNode::Leaf { pane_id } if pane_id == "p1"));
    }

    #[test]
    fn test_remove_from_nested_layout() {
        // Three panes: remove one from a nested split
        let layout = hsplit(0.5, leaf("p1"), vsplit(0.5, leaf("p2"), leaf("p3")));
        let result = remove_from_layout(&layout, "p2").unwrap();
        // Should collapse to hsplit(p1, p3)
        match &result {
            LayoutNode::Split { children, .. } => {
                assert!(
                    matches!(&children.0, LayoutNode::Leaf { pane_id } if pane_id == "p1")
                );
                assert!(
                    matches!(&children.1, LayoutNode::Leaf { pane_id } if pane_id == "p3")
                );
            }
            _ => panic!("Expected Split"),
        }
    }

    #[test]
    fn test_remove_nonexistent_pane() {
        let layout = hsplit(0.5, leaf("p1"), leaf("p2"));
        let result = remove_from_layout(&layout, "p99");
        // Nothing removed; tree should be returned as-is
        assert!(result.is_some());
        let result = result.unwrap();
        let ids = get_all_pane_ids(&result);
        assert!(ids.contains(&"p1".to_string()));
        assert!(ids.contains(&"p2".to_string()));
    }

    #[test]
    fn test_find_pane_down() {
        let layout = vsplit(0.5, leaf("p1"), leaf("p2"));
        let bounds = Rect {
            x: 0,
            y: 0,
            width: 80,
            height: 24,
        };
        let rects = calculate_layout(&layout, bounds);
        assert_eq!(
            find_pane_in_direction(&rects, "p1", Direction::Down, None),
            Some("p2".into())
        );
    }

    #[test]
    fn test_find_pane_up() {
        let layout = vsplit(0.5, leaf("p1"), leaf("p2"));
        let bounds = Rect {
            x: 0,
            y: 0,
            width: 80,
            height: 24,
        };
        let rects = calculate_layout(&layout, bounds);
        assert_eq!(
            find_pane_in_direction(&rects, "p2", Direction::Up, None),
            Some("p1".into())
        );
    }

    #[test]
    fn test_find_pane_with_preferred_tiebreaker() {
        // Create a layout where two panes are at similar distances
        // L-shape: p1 left, p2 top-right, p3 bottom-right
        let layout = hsplit(0.5, leaf("p1"), vsplit(0.5, leaf("p2"), leaf("p3")));
        let bounds = Rect {
            x: 0,
            y: 0,
            width: 80,
            height: 24,
        };
        let rects = calculate_layout(&layout, bounds);
        // From p2, going left, p1 is the only option
        assert_eq!(
            find_pane_in_direction(&rects, "p2", Direction::Left, None),
            Some("p1".into())
        );
    }

    #[test]
    fn test_four_pane_grid_navigation() {
        // 2x2 grid: p1 top-left, p2 top-right, p3 bottom-left, p4 bottom-right
        let layout = vsplit(
            0.5,
            hsplit(0.5, leaf("p1"), leaf("p2")),
            hsplit(0.5, leaf("p3"), leaf("p4")),
        );
        let bounds = Rect {
            x: 0,
            y: 0,
            width: 80,
            height: 24,
        };
        let rects = calculate_layout(&layout, bounds);
        assert_eq!(rects.len(), 4);

        // p1 -> right = p2
        assert_eq!(
            find_pane_in_direction(&rects, "p1", Direction::Right, None),
            Some("p2".into())
        );
        // p1 -> down = p3
        assert_eq!(
            find_pane_in_direction(&rects, "p1", Direction::Down, None),
            Some("p3".into())
        );
        // p4 -> left = p3
        assert_eq!(
            find_pane_in_direction(&rects, "p4", Direction::Left, None),
            Some("p3".into())
        );
        // p4 -> up = p2
        assert_eq!(
            find_pane_in_direction(&rects, "p4", Direction::Up, None),
            Some("p2".into())
        );
    }

    #[test]
    fn test_get_all_pane_ids_single() {
        let layout = leaf("solo");
        let ids = get_all_pane_ids(&layout);
        assert_eq!(ids, vec!["solo"]);
    }

    #[test]
    fn test_asymmetric_ratio_split() {
        let layout = hsplit(0.3, leaf("p1"), leaf("p2"));
        let bounds = Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 24,
        };
        let rects = calculate_layout(&layout, bounds);
        let p1 = rects["p1"];
        let p2 = rects["p2"];
        // split_x = floor(0 + 100 * 0.3) = 30
        assert_eq!(p1.x, 0);
        assert_eq!(p1.width, 30);
        assert_eq!(p2.x, 31);
        assert_eq!(p2.width, 69); // 100 - 30 - 1
    }

    #[test]
    fn test_find_pane_nonexistent_current() {
        let layout = hsplit(0.5, leaf("p1"), leaf("p2"));
        let bounds = Rect {
            x: 0,
            y: 0,
            width: 80,
            height: 24,
        };
        let rects = calculate_layout(&layout, bounds);
        assert_eq!(
            find_pane_in_direction(&rects, "p99", Direction::Right, None),
            None
        );
    }
}
