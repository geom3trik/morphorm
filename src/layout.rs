use crate::Cache;
use crate::Hierarchy;
use crate::Node;
use crate::{Direction, GeometryChanged, LayoutType, PositionType, Units};

use smallvec::SmallVec;

#[derive(Debug, Clone, Copy)]
enum Axis {
    Before,
    Size,
    After,
}

#[derive(Clone, Copy)]
pub struct ComputedData<N: for<'w> Node<'w>> {
    node: N,

    value: f32,
    min: f32,
    max: f32,
    axis: Axis,
}

/// Perform a layout calculation on the visual tree of nodes, the resulting positions and sizes are stored within the provided cache
pub fn layout<'a, 'b, C, H>(
    cache: &mut C,
    hierarchy: &'a H,
    store: &'a <<H as Hierarchy<'a>>::Item as Node<'a>>::Data,
    sublayout: &'b mut <<H as Hierarchy<'a>>::Item as Node<'a>>::Sublayout,
) where
    C: Cache<Item = <H as Hierarchy<'a>>::Item>,
    H: Hierarchy<'a>,
{
    // Step 1 - Determine fist and last parent-directed child of each node and cache it
    // This needs to be done at least once before the rest of layout and when the position_type of a node changes
    for parent in hierarchy.down_iter() {
        // Skip non-visible nodes
        let visible = cache.visible(parent);
        if !visible {
            continue;
        }

        // Reset the sum and max for the parent
        cache.set_child_width_sum(parent, 0.0);
        cache.set_child_height_sum(parent, 0.0);
        cache.set_child_width_max(parent, 0.0);
        cache.set_child_height_max(parent, 0.0);

        cache.set_geo_changed(parent, GeometryChanged::POSX_CHANGED, false);
        cache.set_geo_changed(parent, GeometryChanged::POSY_CHANGED, false);
        cache.set_geo_changed(parent, GeometryChanged::WIDTH_CHANGED, false);
        cache.set_geo_changed(parent, GeometryChanged::HEIGHT_CHANGED, false);

        let mut found_first = false;
        let mut last_child = None;

        for node in hierarchy.child_iter(parent) {
            // Skip non-visible nodes
            let visible = cache.visible(node);
            if !visible {
                continue;
            }

            cache.set_stack_first_child(node, false);
            cache.set_stack_last_child(node, false);

            let position_type = node.position_type(store).unwrap_or_default();

            match position_type {
                PositionType::ParentDirected => {
                    if !found_first {
                        found_first = true;
                        cache.set_stack_first_child(node, true);
                    }
                    last_child = Some(node);
                }

                PositionType::SelfDirected => {
                    cache.set_stack_first_child(node, true);
                    cache.set_stack_last_child(node, true);
                }
            }
        }

        if let Some(last_child) = last_child {
            cache.set_stack_last_child(last_child, true);
        }
    }

    // Step 2 - Iterate up the hierarchy
    // This step is required to determine the sum and max width/height of child nodes
    // to determine the width/height of parent nodes when set to Auto
    for node in hierarchy.up_iter() {
        // Skip non-visible nodes
        let visible = cache.visible(node);
        if !visible {
            continue;
        }

        let parent = hierarchy.parent(node);

        step2(node, parent, cache, store, Direction::X);
        step2(node, parent, cache, store, Direction::Y);
    }

    // Step 3 - Iterate down the hierarchy
    for parent in hierarchy.down_iter() {
        let visible = cache.visible(parent);
        if !visible {
            continue;
        }

        let parent_layout_type = parent.layout_type(store).unwrap_or_default();

        match parent_layout_type {
            LayoutType::Row => {
                step3_row_col(parent, cache, hierarchy, store, sublayout, Direction::X, true);
                step3_row_col(parent, cache, hierarchy, store, sublayout, Direction::Y, false);
            }

            LayoutType::Column => {
                step3_row_col(parent, cache, hierarchy, store, sublayout, Direction::Y, true);
                step3_row_col(parent, cache, hierarchy, store, sublayout, Direction::X, false);
            }

            LayoutType::Grid => step3_grid(parent, cache, hierarchy, store),
        }
    }
}

pub fn step2<'a, C, N>(
    node: N,
    parent: Option<N>,
    cache: &mut C,
    store: &'a N::Data,
    dir: Direction,
) where
    C: Cache<Item = N>,
    N: Node<'a>,
{
    let parent_layout_type =
        parent.map_or(None, |parent| parent.layout_type(store)).unwrap_or_default();
    let layout_type = node.layout_type(store).unwrap_or_default();

    let child_before =
        parent.map_or(None, |parent| parent.child_before(store, dir)).unwrap_or_default();
    let child_after =
        parent.map_or(None, |parent| parent.child_after(store, dir)).unwrap_or_default();
    let row_col_between =
        parent.map_or(None, |parent| parent.row_col_between(store, dir)).unwrap_or_default();
    let mut before = node.before(store, dir).unwrap_or_default();
    let mut after = node.after(store, dir).unwrap_or_default();
    let min_before = node.min_before(store, dir).unwrap_or_default().value_or(0.0, -f32::MAX);
    let max_before = node.max_before(store, dir).unwrap_or_default().value_or(f32::MAX, f32::MAX);
    let min_after = node.min_after(store, dir).unwrap_or_default().value_or(0.0, -f32::MAX);
    let max_after = node.max_after(store, dir).unwrap_or_default().value_or(f32::MAX, f32::MAX);
    let size = node.size(store, dir).unwrap_or(Units::Stretch(1.0));

    // integrate content_width and content_height into the child max and sum
    // this means that if a node has both content and children (weird!) they should overlap
    let content_size = node.content_size(store, dir).unwrap_or_default();
    cache.set_child_size_max(node, cache.child_size_max(node, dir).max(content_size), dir);
    cache.set_child_size_sum(node, cache.child_size_sum(node, dir).max(content_size), dir);

    // If Auto, then set the minimum height to be at least the height_sum/height_max/col_max of the children (depending on layout type)
    let mut min_size = node
        .min_size(store, dir)
        .unwrap_or_default()
        .value_or(0.0, cache.child_size_layout(node, dir, layout_type));
    min_size = min_size.clamp(0.0, f32::MAX);

    let mut max_size = node.max_size(store, dir).unwrap_or_default().value_or(f32::MAX, f32::MAX);
    max_size = max_size.max(min_size);

    let border_before = node.border_before(store, dir).unwrap_or_default().value_or(0.0, 0.0);
    let border_after = node.border_after(store, dir).unwrap_or_default().value_or(0.0, 0.0);

    // If left/right/top/bottom are Auto then the parent child_left/child_right/child_top/child_bottom overrides them
    // The override is also dependent on position in stack (first, last, other) and layout type
    if let Some(layout_dir) = parent_layout_type.direction() {
        if layout_dir == dir {
            if before == Units::Auto {
                if cache.stack_first_child(node) {
                    before = child_before;
                } else {
                    before = row_col_between;
                }
            }
            if after == Units::Auto {
                if cache.stack_last_child(node) {
                    after = child_after;
                }
            }
        } else {
            if before == Units::Auto {
                before = child_before;
            }
            if after == Units::Auto {
                after = child_after;
            }
        }
    }

    let mut new_before = 0.0;
    let mut new_size = 0.0;
    let mut new_after = 0.0;
    let mut used_space = 0.0;

    match parent_layout_type {
        LayoutType::Column | LayoutType::Row => {
            match before {
                Units::Pixels(val) => {
                    new_before = val.clamp(min_before, max_before);
                    used_space += new_before;
                }

                Units::Stretch(_) => {
                    used_space += min_before.clamp(0.0, f32::MAX);
                }

                _ => {}
            }

            match size {
                Units::Pixels(val) => {
                    new_size = val.clamp(min_size, max_size);
                    used_space += new_size;
                }

                Units::Auto => {
                    new_size = cache.child_size_layout(node, dir, layout_type);

                    new_size = new_size.clamp(min_size, max_size);

                    new_size += border_before + border_after;

                    used_space += new_size;
                }

                Units::Stretch(_) => {
                    used_space += min_size;
                }

                _ => {}
            }

            match after {
                Units::Pixels(val) => {
                    new_after = val.clamp(min_after, max_after);
                    used_space += new_after;
                }

                Units::Stretch(_) => {
                    used_space += min_after.clamp(0.0, f32::MAX);
                }

                _ => {}
            }

            let position_type = node.position_type(store).unwrap_or_default();

            cache.set_new_size(node, new_size, dir);
            cache.set_before(node, new_before, dir);
            cache.set_after(node, new_after, dir);

            if let Some(parent) = parent {
                if position_type == PositionType::ParentDirected {
                    cache.set_child_size_sum(
                        parent,
                        cache.child_size_sum(parent, dir) + used_space,
                        dir,
                    );

                    cache.set_child_size_max(
                        parent,
                        used_space.max(cache.child_size_max(parent, dir)),
                        dir,
                    );
                }
            }
        }

        LayoutType::Grid => {
            // TODO
        }
    }
}

pub fn step3_row_col<'a, 'b, C, H>(
    parent: <H as Hierarchy<'a>>::Item,
    cache: &mut C,
    hierarchy: &'a H,
    store: &'a <<H as Hierarchy<'a>>::Item as Node<'a>>::Data,
    sublayout: &'b mut <<H as Hierarchy<'a>>::Item as Node<'a>>::Sublayout,
    dir: Direction,
    primary: bool,
) where
    C: Cache<Item = <H as Hierarchy<'a>>::Item>,
    H: Hierarchy<'a>,
{
    let parent_layout_type = parent.layout_type(store).unwrap_or_default();
    let child_before = parent.child_before(store, dir).unwrap_or_default();
    let child_after = parent.child_after(store, dir).unwrap_or_default();

    let row_col_between = parent.row_col_between(store, dir).unwrap_or_default();

    let mut parent_size = cache.new_size(parent, dir);
    let parent_width_hard = cache.new_width(parent);

    let parent_border_before =
        parent.border_before(store, dir).unwrap_or_default().value_or(parent_width_hard, 0.0);
    let parent_border_after =
        parent.border_after(store, dir).unwrap_or_default().value_or(parent_width_hard, 0.0);

    parent_size -= parent_border_before + parent_border_after;

    let mut parent_free_space = parent_size;
    let mut parent_stretch_sum = 0.0;

    let mut axis = SmallVec::<[ComputedData<<H as Hierarchy>::Item>; 3]>::new();

    // ////////////////////////////////
    // Calculate inflexible children //
    ///////////////////////////////////
    for node in hierarchy.child_iter(parent) {
        let visible = cache.visible(node);
        if !visible {
            continue;
        }

        let layout_type = node.layout_type(store).unwrap_or_default();

        let mut before = node.before(store, dir).unwrap_or_default();
        let mut after = node.after(store, dir).unwrap_or_default();

        let min_before =
            node.min_before(store, dir).unwrap_or_default().value_or(parent_size, -f32::MAX);
        let max_before =
            node.max_before(store, dir).unwrap_or_default().value_or(parent_size, f32::MAX);
        let min_after =
            node.min_after(store, dir).unwrap_or_default().value_or(parent_size, -f32::MAX);
        let max_after =
            node.max_after(store, dir).unwrap_or_default().value_or(parent_size, f32::MAX);

        let size = node.size(store, dir).unwrap_or(Units::Stretch(1.0));

        let auto_size = content_size_smart(store, sublayout, cache, node, parent, dir, layout_type, primary);

        let mut min_size =
            node.min_size(store, dir).unwrap_or_default().value_or(parent_size, auto_size);
        min_size = min_size.clamp(0.0, f32::MAX);

        let mut max_size = node
            .max_size(store, dir)
            .unwrap_or(Units::Pixels(f32::MAX))
            .value_or(parent_size, auto_size);
        max_size = max_size.max(min_size);

        let border_before =
            node.border_before(store, dir).unwrap_or_default().value_or(parent_width_hard, 0.0);
        let border_after =
            node.border_after(store, dir).unwrap_or_default().value_or(parent_width_hard, 0.0);

        let position_type = node.position_type(store).unwrap_or_default();

        // Parent overrides
        if let Some(layout_dir) = parent_layout_type.direction() {
            if layout_dir == dir {
                if before == Units::Auto {
                    if cache.stack_first_child(node) {
                        before = child_before;
                    } else {
                        before = row_col_between;
                    }
                }
                if after == Units::Auto {
                    if cache.stack_last_child(node) {
                        after = child_after;
                    }
                }
            } else {
                if before == Units::Auto {
                    before = child_before;
                }
                if after == Units::Auto {
                    after = child_after;
                }
            }
        }

        let mut stretch_sum = 0.0;
        let mut free_space = parent_size;

        let new_before = incorperate_axis(
            before,
            parent_size,
            min_before,
            max_before,
            &mut free_space,
            &mut stretch_sum,
            Axis::Before,
            &mut axis,
            node,
            0.0,
        );
        let new_size = incorperate_axis(
            size,
            parent_size,
            min_size,
            max_size,
            &mut free_space,
            &mut stretch_sum,
            Axis::Size,
            &mut axis,
            node,
            {
                let auto = auto_size.clamp(min_size, max_size);
                auto + border_before + border_after
            },
        );
        let new_after = incorperate_axis(
            after,
            parent_size,
            min_after,
            max_after,
            &mut free_space,
            &mut stretch_sum,
            Axis::After,
            &mut axis,
            node,
            0.0,
        );

        cache.set_new_size(node, new_size, dir);
        cache.set_before(node, new_before, dir);
        cache.set_after(node, new_after, dir);

        if position_type == PositionType::ParentDirected {
            parent_free_space -= parent_size - free_space;
            parent_stretch_sum += stretch_sum;
        }

        cache.set_free_space(node, free_space, dir);
        cache.set_stretch_sum(node, stretch_sum, dir);
    }

    if parent_stretch_sum == 0.0 {
        parent_stretch_sum = 1.0;
    }

    // Sort the stretch elements in each axis by the maximum size
    axis.sort_by(|a, b| b.min.partial_cmp(&a.min).unwrap());

    let mut stretch_sum = 0.0;
    let mut free_space = 0.0;

    /////////////////////////////////////
    // Calculate flexible space & size //
    /////////////////////////////////////
    for computed_data in axis.iter() {
        let node = computed_data.node.clone();

        let position_type = node.position_type(store).unwrap_or_default();

        match position_type {
            PositionType::SelfDirected => {
                free_space = cache.free_space(node, dir);
                stretch_sum = cache.stretch_sum(node, dir);
            }

            PositionType::ParentDirected => {
                if let Some(parent_layout_dir) = parent_layout_type.direction() {
                    if parent_layout_dir == dir {
                        stretch_sum = parent_stretch_sum;
                        free_space = parent_free_space;
                    } else {
                        free_space = cache.free_space(node, dir);
                        stretch_sum = cache.stretch_sum(node, dir);
                    }
                }
            }
        }

        // Prevent a divide by zero when the stretch sum is 0
        if stretch_sum == 0.0 {
            stretch_sum = 1.0;
        }

        // Compute the new left/width/height based on free space, stretch factor, and stretch_sum
        #[cfg(feature = "rounding")]
        let mut new_value = (free_space * computed_data.value / stretch_sum).round();
        #[cfg(not(feature = "rounding"))]
        let mut new_value = free_space * computed_data.value / stretch_sum;

        // Clamp the new left/width/right to be between min_ left/width/right and max_ left/width/right
        new_value = new_value.clamp(computed_data.min, computed_data.max);

        // Could perhaps replace this with a closure
        match computed_data.axis {
            Axis::Before => cache.set_before(node, new_value, dir),
            Axis::Size => cache.set_new_size(node, new_value, dir),
            Axis::After => cache.set_after(node, new_value, dir),
        }

        match position_type {
            PositionType::SelfDirected => {
                cache.set_stretch_sum(node, stretch_sum - computed_data.value, dir);
                cache.set_free_space(node, free_space - new_value, dir);
            }

            PositionType::ParentDirected => {
                if let Some(parent_layout_dir) = parent_layout_type.direction() {
                    if parent_layout_dir == dir {
                        parent_free_space -= new_value;
                        parent_stretch_sum -= computed_data.value;
                    } else {
                        cache.set_stretch_sum(node, stretch_sum - computed_data.value, dir);
                        cache.set_free_space(node, free_space - new_value, dir);
                    }
                }
            }
        }
    }

    let mut current_pos = 0.0;
    let parent_pos = cache.pos(parent, dir) + parent_border_before;

    ///////////////////////
    // Position Children //
    ///////////////////////
    for node in hierarchy.child_iter(parent) {
        let visible = cache.visible(node);
        if !visible {
            continue;
        }

        let before = cache.before(node, dir);
        let after = cache.after(node, dir);

        let new_size = cache.new_size(node, dir);

        let position_type = node.position_type(store).unwrap_or_default();

        let new_pos = match position_type {
            PositionType::SelfDirected => parent_pos + before,

            PositionType::ParentDirected => {
                let new_pos = parent_pos + current_pos + before;

                if let Some(parent_layout_dir) = parent_layout_type.direction() {
                    if parent_layout_dir == dir {
                        current_pos += before + new_size + after;
                    }
                }

                new_pos
            }
        };

        if new_pos != cache.pos(node, dir) {
            cache.set_geo_changed(node, GeometryChanged::pos_changed(dir), true);
        }

        if new_size != cache.size(node, dir) {
            cache.set_geo_changed(node, GeometryChanged::size_changed(dir), true);
        }

        cache.set_pos(node, new_pos, dir);
        cache.set_size(node, new_size, dir);
    }
}

pub fn step3_grid<'a, C, H>(
    parent: <H as Hierarchy<'a>>::Item,
    cache: &mut C,
    hierarchy: &'a H,
    store: &'a <<H as Hierarchy<'a>>::Item as Node<'a>>::Data,
) where
    C: Cache<Item = <H as Hierarchy<'a>>::Item>,
    H: Hierarchy<'a>,
{
    let parent_width = cache.new_width(parent);
    let parent_height = cache.new_height(parent);

    /////////////////////////////////////////////////////
    // Determine Size of non-flexible rows and columns //
    /////////////////////////////////////////////////////
    let grid_rows = parent.grid_rows(store).unwrap_or_default();
    let grid_cols = parent.grid_cols(store).unwrap_or_default();

    let mut row_heights = vec![(0.0, 0.0); 2 * grid_rows.len() + 2];
    let mut col_widths = vec![(0.0, 0.0,); 2 * grid_cols.len() + 2];

    let row_heights_len = row_heights.len();
    let col_widths_len = col_widths.len();

    let mut col_free_space = parent_width;
    let mut row_free_space = parent_height;

    let mut row_stretch_sum = 0.0;
    let mut col_stretch_sum = 0.0;

    let row_between = parent.row_between(store).unwrap_or_default();
    let col_between = parent.col_between(store).unwrap_or_default();

    let child_left = parent.child_left(store).unwrap_or_default();
    let child_right = parent.child_right(store).unwrap_or_default();
    let child_top = parent.child_top(store).unwrap_or_default();
    let child_bottom = parent.child_bottom(store).unwrap_or_default();

    match child_top {
        Units::Pixels(val) => {
            row_heights[0].1 = val;
            row_free_space -= val;
        }

        Units::Stretch(val) => {
            row_stretch_sum += val;
        }

        _ => {}
    }

    match child_bottom {
        Units::Pixels(val) => {
            row_heights[row_heights_len - 1].1 = val;
            row_free_space -= val;
        }

        Units::Stretch(val) => {
            row_stretch_sum += val;
        }

        _ => {}
    }

    match child_left {
        Units::Pixels(val) => {
            col_widths[0].1 = val;
            col_free_space -= val;
        }

        Units::Stretch(val) => {
            col_stretch_sum += val;
        }

        _ => {}
    }

    match child_right {
        Units::Pixels(val) => {
            col_widths[col_widths_len - 1].1 = val;
            col_free_space -= val;
        }

        Units::Stretch(val) => {
            col_stretch_sum += val;
        }

        _ => {}
    }

    for (i, row) in grid_rows.iter().enumerate() {
        let row_index = 2 * i + 1;

        match row {
            &Units::Pixels(val) => {
                row_heights[row_index].1 = val;
                row_free_space -= val;
            }

            &Units::Stretch(val) => {
                row_stretch_sum += val;
            }

            _ => {}
        }

        if i < grid_rows.len() - 1 {
            let gutter_index = 2 * i + 2;
            match row_between {
                Units::Pixels(val) => {
                    row_heights[gutter_index].1 = val;
                    row_free_space -= val;
                }

                Units::Stretch(val) => {
                    row_stretch_sum += val;
                }

                _ => {}
            }
        }
    }

    for (i, col) in grid_cols.iter().enumerate() {
        let col_index = 2 * i + 1;
        match col {
            &Units::Pixels(val) => {
                col_widths[col_index].1 = val;
                col_free_space -= val;
            }

            &Units::Stretch(val) => {
                col_stretch_sum += val;
            }

            _ => {}
        }

        if i < grid_cols.len() - 1 {
            let gutter_index = 2 * i + 2;
            match col_between {
                Units::Pixels(val) => {
                    col_widths[gutter_index].1 = val;
                    col_free_space -= val;
                }

                Units::Stretch(val) => {
                    col_stretch_sum += val;
                }

                _ => {}
            }
        }
    }

    if row_stretch_sum == 0.0 {
        row_stretch_sum = 1.0;
    }
    if col_stretch_sum == 0.0 {
        col_stretch_sum = 1.0;
    }

    /////////////////////////////////////////////////
    // Determine Size of flexible rows and columns //
    /////////////////////////////////////////////////

    match child_top {
        Units::Stretch(val) => {
            #[cfg(feature = "rounding")]
            {
                row_heights[0].1 = (row_free_space * val / row_stretch_sum).round();
            }
            #[cfg(not(feature = "rounding"))]
            {
                row_heights[0].1 = row_free_space * val / row_stretch_sum;
            }
        }

        _ => {}
    }

    match child_bottom {
        Units::Stretch(val) => {
            #[cfg(feature = "rounding")]
            {
                row_heights[row_heights_len - 1].1 =
                    (row_free_space * val / row_stretch_sum).round();
            }
            #[cfg(not(feature = "rounding"))]
            {
                row_heights[row_heights_len - 1].1 = row_free_space * val / row_stretch_sum;
            }
        }

        _ => {}
    }

    match child_left {
        Units::Stretch(val) => {
            #[cfg(feature = "rounding")]
            {
                col_widths[0].1 = (col_free_space * val / col_stretch_sum).round();
            }
            #[cfg(not(feature = "rounding"))]
            {
                col_widths[0].1 = col_free_space * val / col_stretch_sum;
            }
        }

        _ => {}
    }

    match child_right {
        Units::Stretch(val) => {
            #[cfg(feature = "rounding")]
            {
                col_widths[col_widths_len - 1].1 = (col_free_space * val / col_stretch_sum).round();
            }
            #[cfg(not(feature = "rounding"))]
            {
                col_widths[col_widths_len - 1].1 = col_free_space * val / col_stretch_sum;
            }
        }

        _ => {}
    }

    let mut current_row_pos = cache.posy(parent) + row_heights[0].1;
    let mut current_col_pos = cache.posx(parent) + col_widths[0].1;

    for (i, row) in grid_rows.iter().enumerate() {
        let row_index = 2 * i + 1;
        match row {
            &Units::Stretch(val) => {
                #[cfg(feature = "rounding")]
                {
                    row_heights[row_index].1 = (row_free_space * val / row_stretch_sum).round();
                }
                #[cfg(not(feature = "rounding"))]
                {
                    row_heights[row_index].1 = row_free_space * val / row_stretch_sum;
                }
            }

            _ => {}
        }

        row_heights[row_index].0 = current_row_pos;
        current_row_pos += row_heights[row_index].1;

        if i < grid_rows.len() - 1 {
            let gutter_index = 2 * i + 2;
            match row_between {
                Units::Stretch(val) => {
                    #[cfg(feature = "rounding")]
                    {
                        row_heights[gutter_index].1 =
                            (row_free_space * val / row_stretch_sum).round();
                    }
                    #[cfg(not(feature = "rounding"))]
                    {
                        row_heights[gutter_index].1 = row_free_space * val / row_stretch_sum;
                    }
                }

                _ => {}
            }

            row_heights[gutter_index].0 = current_row_pos;
            current_row_pos += row_heights[gutter_index].1;
        }
    }
    let row_heights_len = row_heights.len() - 1;
    row_heights[row_heights_len - 1].0 = current_row_pos;

    for (i, col) in grid_cols.iter().enumerate() {
        let col_index = 2 * i + 1;

        match col {
            &Units::Stretch(val) => {
                #[cfg(feature = "rounding")]
                {
                    col_widths[col_index].1 = (col_free_space * val / col_stretch_sum).round();
                }
                #[cfg(not(feature = "rounding"))]
                {
                    col_widths[col_index].1 = col_free_space * val / col_stretch_sum;
                }
            }

            _ => {}
        }

        col_widths[col_index].0 = current_col_pos;
        current_col_pos += col_widths[col_index].1;

        if i < grid_cols.len() - 1 {
            let gutter_index = 2 * i + 2;
            match col_between {
                Units::Stretch(val) => {
                    #[cfg(feature = "rounding")]
                    {
                        col_widths[gutter_index].1 =
                            (col_free_space * val / col_stretch_sum).round();
                    }
                    #[cfg(not(feature = "rounding"))]
                    {
                        col_widths[gutter_index].1 = col_free_space * val / col_stretch_sum;
                    }
                }

                _ => {}
            }

            col_widths[gutter_index].0 = current_col_pos;
            current_col_pos += col_widths[gutter_index].1;
        }
    }

    let col_widths_len = col_widths.len() - 1;
    col_widths[col_widths_len - 1].0 = current_col_pos;

    ///////////////////////////////////////////////////
    // Position and Size child nodes within the grid //
    ///////////////////////////////////////////////////
    for node in hierarchy.child_iter(parent) {
        let visible = cache.visible(node);
        if !visible {
            continue;
        }

        let row_start = 2 * node.row_index(store).unwrap_or_default() + 1;
        let row_span = 2 * node.row_span(store).unwrap_or(1) - 1;
        let row_end = row_start + row_span;

        let col_start = 2 * node.col_index(store).unwrap_or_default() + 1;
        let col_span = 2 * node.col_span(store).unwrap_or(1) - 1;
        let col_end = col_start + col_span;

        let new_posx = col_widths[col_start].0;
        let new_width = col_widths[col_end].0 - new_posx;

        let new_posy = row_heights[row_start].0;
        let new_height = row_heights[row_end].0 - new_posy;

        if new_posx != cache.posx(node) {
            cache.set_geo_changed(node, GeometryChanged::POSX_CHANGED, true);
        }

        if new_posy != cache.posy(node) {
            cache.set_geo_changed(node, GeometryChanged::POSY_CHANGED, true);
        }

        if new_width != cache.width(node) {
            cache.set_geo_changed(node, GeometryChanged::WIDTH_CHANGED, true);
        }

        if new_height != cache.height(node) {
            cache.set_geo_changed(node, GeometryChanged::HEIGHT_CHANGED, true);
        }

        cache.set_posx(node, new_posx);
        cache.set_posy(node, new_posy);
        cache.set_width(node, new_width);
        cache.set_height(node, new_height);

        cache.set_new_width(node, cache.width(node));
        cache.set_new_height(node, cache.height(node));
    }
}

fn incorperate_axis<N: Clone + for<'w> Node<'w>>(
    units: Units,
    parent_size: f32,
    min: f32,
    max: f32,
    free_space: &mut f32,
    stretch_sum: &mut f32,
    axis: Axis,
    axis_buf: &mut SmallVec<[ComputedData<N>; 3]>,
    node: N,
    auto_size: f32,
) -> f32 {
    match units {
        Units::Pixels(val) => {
            let new = val.clamp(min, max);
            *free_space -= new;
            new
        }

        Units::Percentage(val) => {
            #[cfg(feature = "rounding")]
            let new = ((val / 100.0) * parent_size).round();
            #[cfg(not(feature = "rounding"))]
            let new = (val / 100.0) * parent_size;
            let new = new.clamp(min, max);
            *free_space -= new;
            new
        }

        Units::Stretch(val) => {
            *stretch_sum += val;
            axis_buf.push(ComputedData { node: node.clone(), value: val, min, max, axis });
            0.0
        }

        Units::Auto => {
            *free_space -= auto_size;
            auto_size
        }
    }
}

// this function is called to retrieve the auto size of a child
fn content_size_smart<'a, 'b, C, N>(
    store: &'a N::Data,
    sublayout: &'b mut N::Sublayout,
    cache: &mut C,
    node: N,
    parent: N,
    dir: Direction,
    layout_type: LayoutType,
    primary: bool,
) -> f32
where
    C: Cache<Item = N>,
    N: Node<'a>,
{
    let basic_answer = cache.child_size_layout(node, dir, layout_type);
    // don't care about grids for now
    if layout_type == LayoutType::Grid {
        return basic_answer;
    }

    // first: check if we're computing the primary or secondary direction of this node
    if Some(dir) == layout_type.direction() {
        // primary direction: easy case - just use the computed size from phase 2
        basic_answer
    } else {
        // secondary direction: hard case - need to compute secondary content-size
        let other_dim = if !primary {
            // easy case - we've already computed the other dimension
            cache.size(node, !dir)
        } else {
            // we need to pre-compute the other dimension
            // this is possible because we're going against the layout direction, so there are no
            // other elements that could be taking up stretch space

            let parent_size = cache.new_size(parent, !dir);
            let mut stretch_sum = 0.0;
            let mut width_remaining = parent_size;
            // TODO: min/max-before/after
            match node.before(store, !dir).unwrap_or_default() {
                Units::Pixels(px) => width_remaining -= px,
                Units::Percentage(p) => width_remaining -= p / 100.0 * parent_size,
                Units::Stretch(su) => stretch_sum += su,
                Units::Auto => {} // auto before/after means 0
            }
            match node.size(store, !dir).unwrap_or_default() {
                Units::Pixels(px) => width_remaining -= px,
                Units::Percentage(p) => width_remaining -= p / 100.0 * parent_size,
                Units::Stretch(su) => stretch_sum += su,
                Units::Auto => width_remaining -= cache.child_size_layout(node, !dir, layout_type),
            }
            match node.after(store, !dir).unwrap_or_default() {
                Units::Pixels(px) => width_remaining -= px,
                Units::Percentage(p) => width_remaining -= p / 100.0 * parent_size,
                Units::Stretch(su) => stretch_sum += su,
                Units::Auto => {}
            }
            if stretch_sum == 0.0 {
                stretch_sum = 1.0;
            }
            let other_min = match node.min_size(store, !dir).unwrap_or_default() {
                Units::Pixels(px) => px,
                Units::Percentage(p) => p / 100.0 * parent_size,
                Units::Stretch(su) => width_remaining * su / stretch_sum,
                Units::Auto => cache.child_size_layout(node, !dir, layout_type),
            }
            .clamp(0.0, f32::MAX);
            let other_max = match node.max_size(store, !dir).unwrap_or_default() {
                Units::Pixels(px) => px,
                Units::Percentage(p) => p / 100.0 * parent_size,
                Units::Stretch(su) => width_remaining * su / stretch_sum,
                Units::Auto => cache.child_size_layout(node, !dir, layout_type),
            }
            .clamp(other_min, f32::MAX);
            let other_size = match node.size(store, !dir).unwrap_or_default() {
                Units::Pixels(v) => v,
                Units::Percentage(p) => p / 100.0 * parent_size,
                Units::Stretch(su) => width_remaining * su / stretch_sum,
                Units::Auto => cache.child_size_layout(node, !dir, layout_type),
            }
            .clamp(other_min, other_max);

            other_size
        };

        let computed = node.content_size_secondary(store, sublayout, other_dim, dir).unwrap_or_default();
        computed.clamp(basic_answer, f32::MAX)
    }
}
