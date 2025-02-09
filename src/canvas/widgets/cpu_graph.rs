use std::borrow::Cow;

use crate::{
    app::{layout_manager::WidgetDirection, App},
    canvas::{
        components::{GraphData, TimeGraph},
        drawing_utils::{get_column_widths, get_start_position, should_hide_x_label},
        Painter,
    },
    constants::*,
    data_conversion::ConvertedCpuData,
};

use concat_string::concat_string;

use tui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    terminal::Frame,
    text::Text,
    widgets::{Block, Borders, Row, Table},
};

const CPU_LEGEND_HEADER: [&str; 2] = ["CPU", "Use%"];
const AVG_POSITION: usize = 1;
const ALL_POSITION: usize = 0;

static CPU_LEGEND_HEADER_LENS: [usize; 2] =
    [CPU_LEGEND_HEADER[0].len(), CPU_LEGEND_HEADER[1].len()];

impl Painter {
    pub fn draw_cpu<B: Backend>(
        &self, f: &mut Frame<'_, B>, app_state: &mut App, draw_loc: Rect, widget_id: u64,
    ) {
        if draw_loc.width as f64 * 0.15 <= 6.0 {
            // Skip drawing legend
            if app_state.current_widget.widget_id == (widget_id + 1) {
                if app_state.app_config_fields.left_legend {
                    app_state.move_widget_selection(&WidgetDirection::Right);
                } else {
                    app_state.move_widget_selection(&WidgetDirection::Left);
                }
            }
            self.draw_cpu_graph(f, app_state, draw_loc, widget_id);
            if let Some(cpu_widget_state) = app_state.cpu_state.widget_states.get_mut(&widget_id) {
                cpu_widget_state.is_legend_hidden = true;
            }

            // Update draw loc in widget map
            if app_state.should_get_widget_bounds() {
                if let Some(bottom_widget) = app_state.widget_map.get_mut(&widget_id) {
                    bottom_widget.top_left_corner = Some((draw_loc.x, draw_loc.y));
                    bottom_widget.bottom_right_corner =
                        Some((draw_loc.x + draw_loc.width, draw_loc.y + draw_loc.height));
                }
            }
        } else {
            let (graph_index, legend_index, constraints) =
                if app_state.app_config_fields.left_legend {
                    (
                        1,
                        0,
                        [Constraint::Percentage(15), Constraint::Percentage(85)],
                    )
                } else {
                    (
                        0,
                        1,
                        [Constraint::Percentage(85), Constraint::Percentage(15)],
                    )
                };

            let partitioned_draw_loc = Layout::default()
                .margin(0)
                .direction(Direction::Horizontal)
                .constraints(constraints)
                .split(draw_loc);

            self.draw_cpu_graph(f, app_state, partitioned_draw_loc[graph_index], widget_id);
            self.draw_cpu_legend(
                f,
                app_state,
                partitioned_draw_loc[legend_index],
                widget_id + 1,
            );

            if app_state.should_get_widget_bounds() {
                // Update draw loc in widget map
                if let Some(cpu_widget) = app_state.widget_map.get_mut(&widget_id) {
                    cpu_widget.top_left_corner = Some((
                        partitioned_draw_loc[graph_index].x,
                        partitioned_draw_loc[graph_index].y,
                    ));
                    cpu_widget.bottom_right_corner = Some((
                        partitioned_draw_loc[graph_index].x
                            + partitioned_draw_loc[graph_index].width,
                        partitioned_draw_loc[graph_index].y
                            + partitioned_draw_loc[graph_index].height,
                    ));
                }

                if let Some(legend_widget) = app_state.widget_map.get_mut(&(widget_id + 1)) {
                    legend_widget.top_left_corner = Some((
                        partitioned_draw_loc[legend_index].x,
                        partitioned_draw_loc[legend_index].y,
                    ));
                    legend_widget.bottom_right_corner = Some((
                        partitioned_draw_loc[legend_index].x
                            + partitioned_draw_loc[legend_index].width,
                        partitioned_draw_loc[legend_index].y
                            + partitioned_draw_loc[legend_index].height,
                    ));
                }
            }
        }
    }

    fn draw_cpu_graph<B: Backend>(
        &self, f: &mut Frame<'_, B>, app_state: &mut App, draw_loc: Rect, widget_id: u64,
    ) {
        const Y_BOUNDS: [f64; 2] = [0.0, 100.5];
        const Y_LABELS: [Cow<'static, str>; 2] = [Cow::Borrowed("  0%"), Cow::Borrowed("100%")];

        if let Some(cpu_widget_state) = app_state.cpu_state.widget_states.get_mut(&widget_id) {
            let cpu_data = &app_state.canvas_data.cpu_data;
            let border_style = self.get_border_style(widget_id, app_state.current_widget.widget_id);
            let x_bounds = [0, cpu_widget_state.current_display_time];
            let hide_x_labels = should_hide_x_label(
                app_state.app_config_fields.hide_time,
                app_state.app_config_fields.autohide_time,
                &mut cpu_widget_state.autohide_timer,
                draw_loc,
            );
            let show_avg_cpu = app_state.app_config_fields.show_average_cpu;
            let show_avg_offset = if show_avg_cpu { AVG_POSITION } else { 0 };
            let points = {
                let current_scroll_position = cpu_widget_state.scroll_state.current_scroll_position;
                if current_scroll_position == ALL_POSITION {
                    // This case ensures the other cases cannot have the position be equal to 0.
                    cpu_data
                        .iter()
                        .enumerate()
                        .rev()
                        .map(|(itx, cpu)| {
                            let style = if show_avg_cpu && itx == AVG_POSITION {
                                self.colours.avg_colour_style
                            } else if itx == ALL_POSITION {
                                self.colours.all_colour_style
                            } else {
                                let offset_position = itx - 1; // Because of the all position
                                self.colours.cpu_colour_styles[(offset_position - show_avg_offset)
                                    % self.colours.cpu_colour_styles.len()]
                            };

                            GraphData {
                                points: &cpu.cpu_data[..],
                                style,
                                name: None,
                            }
                        })
                        .collect::<Vec<_>>()
                } else if let Some(cpu) = cpu_data.get(current_scroll_position) {
                    let style = if show_avg_cpu && current_scroll_position == AVG_POSITION {
                        self.colours.avg_colour_style
                    } else {
                        let offset_position = current_scroll_position - 1; // Because of the all position
                        self.colours.cpu_colour_styles[(offset_position - show_avg_offset)
                            % self.colours.cpu_colour_styles.len()]
                    };

                    vec![GraphData {
                        points: &cpu.cpu_data[..],
                        style,
                        name: None,
                    }]
                } else {
                    vec![]
                }
            };

            // TODO: Maybe hide load avg if too long? Or maybe the CPU part.
            let title = if cfg!(target_family = "unix") {
                let load_avg = app_state.canvas_data.load_avg_data;
                let load_avg_str = format!(
                    "─ {:.2} {:.2} {:.2} ",
                    load_avg[0], load_avg[1], load_avg[2]
                );

                concat_string!(" CPU ", load_avg_str).into()
            } else {
                " CPU ".into()
            };

            TimeGraph {
                use_dot: app_state.app_config_fields.use_dot,
                x_bounds,
                hide_x_labels,
                y_bounds: Y_BOUNDS,
                y_labels: &Y_LABELS,
                graph_style: self.colours.graph_style,
                border_style,
                title,
                is_expanded: app_state.is_expanded,
                title_style: self.colours.widget_title_style,
                legend_constraints: None,
            }
            .draw_time_graph(f, draw_loc, &points);
        }
    }

    fn draw_cpu_legend<B: Backend>(
        &self, f: &mut Frame<'_, B>, app_state: &mut App, draw_loc: Rect, widget_id: u64,
    ) {
        let recalculate_column_widths = app_state.should_get_widget_bounds();
        if let Some(cpu_widget_state) = app_state.cpu_state.widget_states.get_mut(&(widget_id - 1))
        {
            cpu_widget_state.is_legend_hidden = false;
            let cpu_data: &mut [ConvertedCpuData] = &mut app_state.canvas_data.cpu_data;
            let cpu_table_state = &mut cpu_widget_state.scroll_state.table_state;
            let is_on_widget = widget_id == app_state.current_widget.widget_id;
            let table_gap = if draw_loc.height < TABLE_GAP_HEIGHT_LIMIT {
                0
            } else {
                app_state.app_config_fields.table_gap
            };
            let start_position = get_start_position(
                usize::from(
                    (draw_loc.height + (1 - table_gap)).saturating_sub(self.table_height_offset),
                ),
                &cpu_widget_state.scroll_state.scroll_direction,
                &mut cpu_widget_state.scroll_state.scroll_bar,
                cpu_widget_state.scroll_state.current_scroll_position,
                app_state.is_force_redraw,
            );
            cpu_table_state.select(Some(
                cpu_widget_state
                    .scroll_state
                    .current_scroll_position
                    .saturating_sub(start_position),
            ));

            let sliced_cpu_data = &cpu_data[start_position..];

            let offset_scroll_index = cpu_widget_state
                .scroll_state
                .current_scroll_position
                .saturating_sub(start_position);
            let show_avg_cpu = app_state.app_config_fields.show_average_cpu;

            // Calculate widths
            if recalculate_column_widths {
                cpu_widget_state.table_width_state.desired_column_widths = vec![6, 4];
                cpu_widget_state.table_width_state.calculated_column_widths = get_column_widths(
                    draw_loc.width,
                    &[None, None],
                    &(CPU_LEGEND_HEADER_LENS
                        .iter()
                        .map(|width| Some(*width as u16))
                        .collect::<Vec<_>>()),
                    &[Some(0.5), Some(0.5)],
                    &(cpu_widget_state
                        .table_width_state
                        .desired_column_widths
                        .iter()
                        .map(|width| Some(*width))
                        .collect::<Vec<_>>()),
                    false,
                );
            }

            let dcw = &cpu_widget_state.table_width_state.desired_column_widths;
            let ccw = &cpu_widget_state.table_width_state.calculated_column_widths;
            let cpu_rows = sliced_cpu_data.iter().enumerate().map(|(itx, cpu)| {
                let mut truncated_name =
                    if let (Some(desired_column_width), Some(calculated_column_width)) =
                        (dcw.get(0), ccw.get(0))
                    {
                        if *desired_column_width > *calculated_column_width {
                            Text::raw(&cpu.short_cpu_name)
                        } else {
                            Text::raw(&cpu.cpu_name)
                        }
                    } else {
                        Text::raw(&cpu.cpu_name)
                    };

                let is_first_column_hidden = if let Some(calculated_column_width) = ccw.get(0) {
                    *calculated_column_width == 0
                } else {
                    false
                };

                let truncated_legend = if is_first_column_hidden && cpu.legend_value.is_empty() {
                    // For the case where we only have room for one column, display "All" in the normally blank area.
                    Text::raw("All")
                } else {
                    Text::raw(&cpu.legend_value)
                };

                if !is_first_column_hidden
                    && itx == offset_scroll_index
                    && itx + start_position == ALL_POSITION
                {
                    truncated_name.patch_style(self.colours.currently_selected_text_style);
                    Row::new(vec![truncated_name, truncated_legend])
                } else {
                    let cpu_string_row = vec![truncated_name, truncated_legend];

                    Row::new(cpu_string_row).style(if itx == offset_scroll_index {
                        self.colours.currently_selected_text_style
                    } else if itx + start_position == ALL_POSITION {
                        self.colours.all_colour_style
                    } else if show_avg_cpu {
                        if itx + start_position == AVG_POSITION {
                            self.colours.avg_colour_style
                        } else {
                            self.colours.cpu_colour_styles[(itx + start_position
                                - AVG_POSITION
                                - 1)
                                % self.colours.cpu_colour_styles.len()]
                        }
                    } else {
                        self.colours.cpu_colour_styles[(itx + start_position - ALL_POSITION - 1)
                            % self.colours.cpu_colour_styles.len()]
                    })
                }
            });

            // Note we don't set highlight_style, as it should always be shown for this widget.
            let border_and_title_style = if is_on_widget {
                self.colours.highlighted_border_style
            } else {
                self.colours.border_style
            };

            // Draw
            f.render_stateful_widget(
                Table::new(cpu_rows)
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .border_style(border_and_title_style),
                    )
                    .header(
                        Row::new(CPU_LEGEND_HEADER.to_vec())
                            .style(self.colours.table_header_style)
                            .bottom_margin(table_gap),
                    )
                    .widths(
                        &(cpu_widget_state
                            .table_width_state
                            .calculated_column_widths
                            .iter()
                            .map(|calculated_width| Constraint::Length(*calculated_width as u16))
                            .collect::<Vec<_>>()),
                    ),
                draw_loc,
                cpu_table_state,
            );
        }
    }
}
