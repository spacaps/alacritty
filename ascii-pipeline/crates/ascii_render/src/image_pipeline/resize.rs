#[derive(Clone, Copy, Debug)]
pub struct TargetGeometry {
    pub columns: u16,
    pub rows: u16,
    pub cell_aspect: f32,
}

#[derive(Clone, Copy, Debug)]
pub enum LayoutPolicy {
    FixedColumns(u16),
    FitViewport { columns: u16, rows: u16, cell_aspect: f32 },
    ScaleToHeight { rows: u16, cell_aspect: f32 },
}

impl LayoutPolicy {
    pub fn derive(
        &self,
        source_width: u32,
        source_height: u32,
        default_aspect: f32,
    ) -> Option<TargetGeometry> {
        if source_width == 0 || source_height == 0 {
            return None;
        }

        let image_ratio = source_height as f32 / source_width as f32;

        match *self {
            LayoutPolicy::FixedColumns(columns) => {
                let columns = columns.max(1);
                let rows = ((image_ratio * columns as f32 * default_aspect).round() as u16).max(1);
                Some(TargetGeometry { columns, rows, cell_aspect: default_aspect })
            },
            LayoutPolicy::FitViewport { columns, rows, cell_aspect } => {
                let mut columns = columns.max(1);
                let mut rows_limit = rows.max(1);
                let mut rows = ((image_ratio * columns as f32 * cell_aspect).round() as u16).max(1);

                if rows > rows_limit {
                    rows = rows_limit;
                    let derived_columns =
                        ((rows as f32) / (image_ratio * cell_aspect)).round() as u16;
                    columns = columns.min(derived_columns.max(1));
                } else {
                    rows_limit = rows;
                }

                Some(TargetGeometry { columns, rows: rows_limit, cell_aspect })
            },
            LayoutPolicy::ScaleToHeight { rows, cell_aspect } => {
                let rows = rows.max(1);
                let columns = ((rows as f32) / (image_ratio * cell_aspect)).round() as u16;
                Some(TargetGeometry { columns: columns.max(1), rows, cell_aspect })
            },
        }
    }
}
