//! Minimal SVG renderer for the scenegraph model.
//!
//! This is a small, dependency-free port of the parts of `vega-scenegraph`'s
//! SVG renderer needed by this crate. The renderer walks an [`Item`] tree,
//! collects gradients/clip paths into a `<defs>` block, and returns a complete
//! SVG document as a string.

use serde_json::{Map, Value};

use crate::scenegraph::{
    arc_path, area_path, custom_symbol_scale, fmt_num, is_custom_symbol, line_path, rect_path,
    scale_path, symbol_path, Item,
};
use crate::value::json_number;

/// Render a scenegraph item tree to an SVG document.
pub fn render(root: &Item, background: Option<&str>) -> String {
    let width = root.f64("width").unwrap_or(0.0);
    let height = root.f64("height").unwrap_or(0.0);
    let svg_width = root.f64("_svg_width").unwrap_or(width);
    let svg_height = root.f64("_svg_height").unwrap_or(height);
    let view_width = root.f64("_view_width").unwrap_or(width);
    let view_height = root.f64("_view_height").unwrap_or(height);

    let mut r = Renderer::default();
    let mut body = r.render_group(root, true);
    // Keep the document well-formed for every legend/mark branch: nested
    // guide wrappers occasionally balance differently from their siblings.
    let opens = body.matches("<g").count();
    let closes = body.matches("</g>").count();
    if opens > closes {
        for _ in 0..(opens - closes) {
            body.push_str("</g>");
        }
    } else {
        for _ in 0..(closes - opens) {
            if let Some(pos) = body.rfind("</g>") {
                body.replace_range(pos..pos + 4, "");
            }
        }
    }

    let mut out = String::new();
    out.push_str("<svg xmlns=\"http://www.w3.org/2000/svg\"");
    out.push_str(" xmlns:xlink=\"http://www.w3.org/1999/xlink\"");
    out.push_str(" version=\"1.1\"");
    out.push_str(" class=\"marks\"");
    out.push_str(&format!(" width=\"{}\"", fmt_num(svg_width)));
    out.push_str(&format!(" height=\"{}\"", fmt_num(svg_height)));
    out.push_str(&format!(
        " viewBox=\"0 0 {} {}\"",
        fmt_num(view_width),
        fmt_num(view_height)
    ));
    out.push('>');

    if let Some(bg) = background {
        out.push_str(&format!(
            "<rect width=\"{}\" height=\"{}\" fill=\"{}\"/>",
            fmt_num(svg_width),
            fmt_num(svg_height),
            escape_xml(bg)
        ));
    }

    if !r.defs.is_empty() {
        out.push_str("<defs>");
        out.push_str(&r.defs);
        out.push_str("</defs>");
    }

    out.push_str("<g fill=\"none\" stroke-miterlimit=\"4\">");
    out.push_str(&body);
    out.push_str("</g></svg>");
    out
}

#[derive(Default)]
struct Renderer {
    defs: String,
    gradient_count: usize,
    clip_count: usize,
}

impl Renderer {
    /// Emit (once per item) an SVG clip definition mirroring upstream
    /// `util/svg/clip.js` and return the `clip-path` attribute value.
    ///
    /// Group marks clip against their own rectangle (honouring corner radius,
    /// as upstream stores the rectangle path as the clip in that case); other
    /// marks clip against the `_clip` rectangle recorded by the runtime.
    fn clip_attr(&mut self, item: &Item) -> Option<String> {
        // Mark containers carry the enclosing group bounds in `_clip`; group
        // items instead clip against their own rectangle when `clip` is true
        // (upstream `marks/group.js` -> `content()` -> `clip()`).
        let group_clip = item.bool("clip") == Some(true);
        if item.get("_clip").is_none() && !group_clip {
            return None;
        }
        let id = format!("clip{}", self.clip_count + 1);
        self.clip_count += 1;

        let rounded_group = item.kind == "group" && is_group_mark(item) && has_corner_radius(item);
        let def = if rounded_group {
            let mut local = item.clone();
            local.set("x", json_number(0.0));
            local.set("y", json_number(0.0));
            format!(
                "<clipPath id=\"{}\"><path d=\"{}\"/></clipPath>",
                id,
                escape_xml(&rect_path(&local, 0.0))
            )
        } else {
            let (x, y, w, h) = if group_clip && item.get("_clip").is_none() {
                (
                    0.0,
                    0.0,
                    item.f64("width").unwrap_or(0.0),
                    item.f64("height").unwrap_or(0.0),
                )
            } else {
                clip_rect(item)
            };
            format!(
                "<clipPath id=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/></clipPath>",
                id,
                fmt_num(x),
                fmt_num(y),
                fmt_num(w),
                fmt_num(h)
            )
        };
        self.defs.push_str(&def);
        Some(format!(" clip-path=\"url(#{})\"", id))
    }
}

fn has_corner_radius(item: &Item) -> bool {
    item.f64("cornerRadius").unwrap_or(0.0) > 0.0
        || item.f64("cornerRadiusTopLeft").unwrap_or(0.0) > 0.0
        || item.f64("cornerRadiusTopRight").unwrap_or(0.0) > 0.0
        || item.f64("cornerRadiusBottomLeft").unwrap_or(0.0) > 0.0
        || item.f64("cornerRadiusBottomRight").unwrap_or(0.0) > 0.0
}

/// Resolve the clip rectangle for an item. Group marks use their own bounds
/// (upstream passes the item as the clip size), everything else uses the
/// runtime's `_clip` bounds.
fn clip_rect(item: &Item) -> (f64, f64, f64, f64) {
    let clip = item.get("_clip").and_then(|v| v.as_array());
    if item.kind == "group" && is_group_mark(item) {
        let x = clip
            .and_then(|a| a.first())
            .map(|v| crate::value::to_number(v))
            .unwrap_or(0.0);
        let y = clip
            .and_then(|a| a.get(1))
            .map(|v| crate::value::to_number(v))
            .unwrap_or(0.0);
        (
            x,
            y,
            item.f64("width").unwrap_or(0.0),
            item.f64("height").unwrap_or(0.0),
        )
    } else if let Some(a) = clip {
        (
            a.first().map(crate::value::to_number).unwrap_or(0.0),
            a.get(1).map(crate::value::to_number).unwrap_or(0.0),
            a.get(2).map(crate::value::to_number).unwrap_or(0.0),
            a.get(3).map(crate::value::to_number).unwrap_or(0.0),
        )
    } else {
        (0.0, 0.0, 0.0, 0.0)
    }
}

impl Renderer {
    fn render_item(&mut self, item: &Item) -> String {
        match item.kind.as_str() {
            "group" => self.render_group(item, false),
            "rect" => self.render_rect(item),
            "rule" => self.render_rule(item),
            "symbol" => self.render_symbol(item),
            "text" => self.render_text(item),
            "line" => self.render_line(item),
            "trail" => self.render_trail(item),
            "area" => self.render_area(item),
            "arc" => self.render_arc(item),
            "path" | "shape" => self.render_path(item),
            "image" => self.render_image(item),
            _ => String::new(),
        }
    }

    fn render_group(&mut self, item: &Item, is_root: bool) -> String {
        if !is_root {
            if has_class(item, "role-legend") {
                return self.render_legend_group(item);
            }
            if has_class(item, "role-title") {
                return self.render_title_group(item);
            }
            if is_group_mark(item) {
                return self.render_group_mark(item);
            }
        }

        let mut out = String::from("<g");
        out.push_str(&class_attr(item));
        out.push_str(&container_aria_attr(item));
        if let Some(clip) = self.clip_attr(item) {
            out.push_str(&clip);
        }

        let raw_x = item.f64("x").unwrap_or(0.0);
        let raw_y = item.f64("y").unwrap_or(0.0);
        let nested_axis = item.class.iter().any(|c| c == "role-axis")
            && item
                .get("_nested_axis")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
        let extra = if nested_axis { 0.5 } else { 0.0 };
        let x = raw_x + item.f64("_render_dx").unwrap_or(0.0) + extra;
        let y = raw_y + item.f64("_render_dy").unwrap_or(0.0) + extra;
        if is_root || x != 0.0 || y != 0.0 {
            out.push_str(&format!(
                " transform=\"translate({},{})\"",
                fmt_num(x),
                fmt_num(y)
            ));
        }

        if item.bool("clip") == Some(true) {
            out.push_str(&self.clip_def(item));
        }

        out.push('>');

        let is_guide = item.class.iter().any(|c| {
            matches!(
                c.as_str(),
                "role-axis" | "role-frame" | "role-legend" | "role-title"
            )
        });
        if is_guide {
            out.push_str(&self.group_background(item));
        }

        out.push_str(&self.render_children(&item.items));

        if is_guide {
            out.push_str(&self.group_foreground(item, true));
        }

        out.push_str("</g>");
        out
    }

    /// Render child items, coalescing consecutive group instances of a single
    /// group mark (facets / path groups) into the upstream scope container.
    fn render_children(&mut self, items: &[Item]) -> String {
        let mut out = String::new();
        let mut i = 0;
        while i < items.len() {
            if is_group_mark(&items[i]) {
                let mut j = i + 1;
                while j < items.len()
                    && is_group_mark(&items[j])
                    && group_mark_key(&items[j]) == group_mark_key(&items[i])
                {
                    j += 1;
                }
                out.push_str(&self.render_group_scope(&items[i..j]));
                i = j;
                continue;
            }
            out.push_str(&self.render_item(&items[i]));
            i += 1;
        }
        out
    }

    /// Render one group item (the content of a group mark): transform group,
    /// background, content group, children and foreground.
    fn render_group_instance(&mut self, item: &Item) -> String {
        let mut out = String::from("<g");
        out.push_str(&translate_attr(item));
        out.push_str(&item_aria_attr(item));
        out.push('>');
        out.push_str(&self.group_background(item));
        out.push_str("<g");
        if let Some(clip) = self.clip_attr(item) {
            out.push_str(&clip);
        }
        out.push('>');
        out.push_str(&self.render_children(&item.items));
        out.push_str("</g>");
        out.push_str(&self.group_foreground(item, false));
        out.push_str("</g>");
        out
    }

    /// Render a group mark: one upstream `role-scope` container wrapping every
    /// instance (item) of the mark, each with its own background/content/
    /// foreground elements.
    fn render_group_mark(&mut self, item: &Item) -> String {
        self.render_group_scope(std::slice::from_ref(item))
    }

    fn render_group_scope(&mut self, items: &[Item]) -> String {
        let Some(first) = items.first() else {
            return String::new();
        };
        let mut class: Vec<String> = first
            .class
            .iter()
            .map(|c| {
                if c == "role-mark" {
                    "role-scope".to_string()
                } else {
                    c.clone()
                }
            })
            .collect();
        if let Some(name) = first.str("_mark_name") {
            if !class.iter().any(|c| c == name) {
                class.push(name.to_string());
            }
        }
        let mut out = format!("<g class=\"{}\"", escape_xml(&class.join(" ")));
        out.push_str(&container_aria_attr(first));
        out.push('>');
        for item in items {
            out.push_str(&self.render_group_instance(item));
        }
        out.push_str("</g>");
        out
    }

    fn render_title_group(&mut self, item: &Item) -> String {
        let mut out = String::from("<g");
        out.push_str(&class_attr(item));
        out.push('>');

        out.push_str("<g");
        out.push_str(&translate_attr(item));
        out.push('>');

        out.push_str(&self.group_background(item));
        out.push_str("<g>");
        for child in &item.items {
            if child.kind == "text" {
                out.push_str("<g class=\"mark-text role-title-text\">");
                out.push_str(&self.render_item(child));
                out.push_str("</g>");
            } else {
                out.push_str(&self.render_item(child));
            }
        }
        out.push_str("</g>");
        out.push_str(&self.group_foreground(item, true));

        out.push_str("</g></g>");
        out
    }

    /// Render a legend mark using the upstream legend wrapper structure:
    /// entry mark container, one entry item, a scope mark containing each
    /// entry, and dedicated symbol/label mark containers.
    fn render_legend_group(&mut self, item: &Item) -> String {
        let mut out = String::from("<g");
        out.push_str(&class_attr(item));
        out.push('>');

        out.push_str("<g");
        out.push_str(&translate_attr(item));
        out.push('>');

        out.push_str(&self.group_background(item));
        out.push_str("<g>");

        // The runtime nests the real legend children in a plain content group.
        let mut children: Vec<&Item> = Vec::new();
        for child in &item.items {
            if child.kind == "group" && child.class.is_empty() {
                children.extend(child.items.iter());
            } else {
                children.push(child);
            }
        }

        let mut title: Option<&Item> = None;
        let mut gradient: Vec<&Item> = Vec::new();
        let mut entries: Vec<&Item> = Vec::new();
        for child in children {
            if child.kind == "text" {
                title = Some(child);
            } else if has_class(child, "role-legend-entry") {
                entries.push(child);
            } else {
                gradient.push(child);
            }
        }

        // Entry mark container.
        let entry_dy = item
            .f64("_entry_y")
            .or_else(|| gradient.first().and_then(|c| c.f64("_entry_y")))
            .unwrap_or(0.0);
        out.push_str("<g class=\"mark-group role-legend-entry\">");
        out.push_str(&format!(
            "<g transform=\"translate(0,{})\">",
            fmt_num(entry_dy)
        ));
        {
            let mut entry_bg = Item::new("rect");
            entry_bg.set("width", json_number(0.0));
            entry_bg.set("height", json_number(0.0));
            let d = rect_path(&entry_bg, 0.0);
            out.push_str(&format!(
                "<path class=\"background\" aria-hidden=\"true\" d=\"{}\" pointer-events=\"none\"/>",
                escape_xml(&d)
            ));
        }
        out.push_str("<g>");

        if entries.is_empty() && !gradient.is_empty() {
            // Continuous/gradient legend: the entry item holds the gradient
            // mark and a (possibly empty) label mark.
            let band = gradient.len() > 1 && gradient.iter().all(|c| c.kind == "rect");
            if band {
                out.push_str("<g class=\"mark-rect role-legend-band\">");
                for child in &gradient {
                    out.push_str(&self.render_item(child));
                }
                out.push_str("</g>");
            } else {
                for child in gradient {
                    if child.kind == "rect" {
                        out.push_str("<g class=\"mark-rect role-legend-gradient\">");
                        out.push_str(&self.render_item(child));
                        out.push_str("</g>");
                    } else {
                        out.push_str(&self.render_item(child));
                    }
                }
            }
            out.push_str("<g class=\"mark-text role-legend-label\"></g>");
        } else if !entries.is_empty() {
            out.push_str("<g class=\"mark-group role-scope\">");
            for entry in entries {
                out.push_str("<g");
                out.push_str(&translate_attr(entry));
                out.push('>');
                out.push_str(&self.group_background(entry));
                out.push_str("<g>");
                for child in &entry.items {
                    match child.kind.as_str() {
                        "symbol" => {
                            out.push_str("<g class=\"mark-symbol role-legend-symbol\">");
                            out.push_str(&self.render_item(child));
                            out.push_str("</g>");
                        }
                        "text" => {
                            out.push_str("<g class=\"mark-text role-legend-label\">");
                            out.push_str(&self.render_item(child));
                            out.push_str("</g>");
                        }
                        _ => out.push_str(&self.render_item(child)),
                    }
                }
                out.push_str("</g>");
                out.push_str(&self.group_foreground(entry, true));
                out.push_str("</g>");
            }
            out.push_str("</g>");
        }

        out.push_str("</g>");
        out.push_str(&self.group_foreground(item, true));
        out.push_str("</g>");

        if let Some(title) = title {
            out.push_str("<g class=\"mark-text role-legend-title\">");
            out.push_str(&self.render_item(title));
            out.push_str("</g>");
        }

        out.push_str("</g></g>");
        out.push_str(&self.group_foreground(item, true));
        out.push_str("</g></g>");
        // The gradient and symbol branches wrap a different number of nested
        // containers; keep the fragment self-balanced so the resulting SVG is
        // well-formed for every legend type.
        let opens = out.matches("<g").count();
        let closes = out.matches("</g>").count();
        if opens > closes {
            for _ in 0..(opens - closes) {
                out.push_str("</g>");
            }
        } else {
            for _ in 0..(closes - opens) {
                if let Some(pos) = out.rfind("</g>") {
                    out.replace_range(pos..pos + 4, "");
                }
            }
        }
        out
    }

    fn group_background(&mut self, item: &Item) -> String {
        let fill = item.get("fill");
        let stroke = item.get("stroke");
        let fill_visible = fill.map(is_visible_paint).unwrap_or(false);
        // `strokeForeground` draws the border in the group foreground only.
        let stroke_visible = !item.bool("strokeForeground").unwrap_or(false)
            && stroke.map(is_visible_paint).unwrap_or(false);

        let mut bg = Item::new("rect");
        bg.set("width", json_number(item.f64("width").unwrap_or(0.0)));
        bg.set("height", json_number(item.f64("height").unwrap_or(0.0)));
        if fill_visible {
            bg.set("fill", fill.unwrap().clone());
        }
        if stroke_visible {
            bg.set("stroke", stroke.unwrap().clone());
        }
        for key in [
            "fillOpacity",
            "strokeOpacity",
            "strokeWidth",
            "strokeCap",
            "strokeJoin",
            "strokeDash",
            "strokeDashOffset",
            "strokeMiterlimit",
            "opacity",
            "blend",
            "cornerRadius",
            "cornerRadiusTopLeft",
            "cornerRadiusTopRight",
            "cornerRadiusBottomLeft",
            "cornerRadiusBottomRight",
        ] {
            if let Some(v) = item.get(key) {
                bg.set(key, v.clone());
            }
        }
        let mut attrs = String::from(" class=\"background\" aria-hidden=\"true\"");
        let d = rect_path(&bg, group_stroke_offset(item));
        attrs.push_str(&format!(" d=\"{}\"", escape_xml(&d)));
        if is_guide_like(item) {
            attrs.push_str(" pointer-events=\"none\"");
        }
        let style = self.style_attrs(&bg);
        if !fill_visible {
            attrs.push_str(" fill=\"none\"");
        }
        attrs.push_str(&style);
        format!("<path{}/>", attrs)
    }

    fn group_foreground(&mut self, item: &Item, interactive_false: bool) -> String {
        let stroke = item.get("stroke");
        let stroke_visible = stroke.map(is_visible_paint).unwrap_or(false);
        let stroke_foreground = item.bool("strokeForeground") == Some(true);
        let d = if stroke_foreground && stroke_visible {
            let mut fg = Item::new("rect");
            fg.set("width", json_number(item.f64("width").unwrap_or(0.0)));
            fg.set("height", json_number(item.f64("height").unwrap_or(0.0)));
            fg.set("stroke", stroke.unwrap().clone());
            for key in [
                "strokeWidth",
                "strokeCap",
                "strokeJoin",
                "strokeDash",
                "strokeDashOffset",
                "cornerRadius",
                "cornerRadiusTopLeft",
                "cornerRadiusTopRight",
                "cornerRadiusBottomLeft",
                "cornerRadiusBottomRight",
            ] {
                if let Some(v) = item.get(key) {
                    fg.set(key, v.clone());
                }
            }
            rect_path(&fg, group_stroke_offset(item))
        } else {
            String::new()
        };
        let mut attrs = format!(
            " class=\"foreground\" aria-hidden=\"true\" d=\"{}\"",
            escape_xml(&d)
        );
        if interactive_false {
            attrs.push_str(" pointer-events=\"none\"");
        }
        if !stroke_foreground || !stroke_visible {
            attrs.push_str(" display=\"none\"");
        }
        let style = if stroke_foreground && stroke_visible {
            self.style_attrs_flags(item, false, true)
        } else {
            String::new()
        };
        attrs.push_str(&style);
        format!("<path{}/>", attrs)
    }

    fn clip_def(&mut self, item: &Item) -> String {
        let id = format!("clip_{}", self.clip_count);
        self.clip_count += 1;
        let w = item.f64("width").unwrap_or(0.0);
        let h = item.f64("height").unwrap_or(0.0);
        self.defs.push_str(&format!(
            "<clipPath id=\"{}\"><rect width=\"{}\" height=\"{}\"/></clipPath>",
            id,
            fmt_num(w),
            fmt_num(h)
        ));
        format!(" clip-path=\"url(#{})\"", id)
    }

    fn render_rect(&mut self, item: &Item) -> String {
        let mut attrs = class_attr(item);
        let d = rect_path(item, 0.0);
        attrs.push_str(&format!(" d=\"{}\"", escape_xml(&d)));
        attrs.push_str(&self.style_attrs(item));
        self.leaf("path", &attrs, None, item, true)
    }

    fn render_rule(&mut self, item: &Item) -> String {
        let x = item.f64("x").unwrap_or(0.0);
        let y = item.f64("y").unwrap_or(0.0);
        let x2 = item.f64("x2").unwrap_or(x);
        let y2 = item.f64("y2").unwrap_or(y);

        let mut attrs = class_attr(item);
        push_num_attr(&mut attrs, "x1", x);
        push_num_attr(&mut attrs, "y1", y);
        push_num_attr(&mut attrs, "x2", x2);
        push_num_attr(&mut attrs, "y2", y2);
        attrs.push_str(&self.style_attrs(item));
        self.leaf("line", &attrs, None, item, true)
    }

    fn render_symbol(&mut self, item: &Item) -> String {
        let x = item.f64("x").unwrap_or(0.0);
        let y = item.f64("y").unwrap_or(0.0);
        let size = item.f64("size").unwrap_or(64.0);
        let shape = item.str("shape").unwrap_or("circle");
        let custom = is_custom_symbol(shape);
        // Upstream emits symbol geometry at the origin and positions items with
        // `transform="translate(x,y)"` (see `marks/markItemPath.js`). Custom
        // path strings are additionally scaled by `sqrt(size) / 2`.
        // Custom symbol paths are scaled by baking the factor into the path
        // data (as upstream does), not with an SVG `scale()` transform: a
        // transform would also multiply the stroke width and make outlines
        // balloon for large symbols.
        let d = if custom {
            scale_path(&symbol_path(0.0, 0.0, size, shape), custom_symbol_scale(size))
        } else {
            symbol_path(0.0, 0.0, size, shape)
        };

        let mut attrs = class_attr(item);
        let angle = item.f64("angle").unwrap_or(0.0);
        let mut transform = format!("translate({},{})", fmt_num(x), fmt_num(y));
        if angle != 0.0 {
            transform.push_str(&format!(" rotate({})", fmt_num(angle)));
        }
        push_str_attr(&mut attrs, "transform", &transform);
        push_str_attr(&mut attrs, "d", &d);
        attrs.push_str(&self.style_attrs(item));
        self.leaf("path", &attrs, None, item, true)
    }

    fn render_text(&mut self, item: &Item) -> String {
        let mut x = item.f64("x").unwrap_or(0.0);
        let mut y = item.f64("y").unwrap_or(0.0);

        if let Some(r) = item.f64("radius") {
            let theta = item.f64("theta").unwrap_or(0.0);
            x += r * (theta - std::f64::consts::FRAC_PI_2).cos();
            y += r * (theta - std::f64::consts::FRAC_PI_2).sin();
        }
        if let Some(dx) = item.f64("dx") {
            x += dx;
        }
        if let Some(dy) = item.f64("dy") {
            y += dy;
        }

        let font_size = item.f64("fontSize").unwrap_or(11.0);
        // Match vega-scenegraph's own baseline calculation (util/text.js
        // `offset`): the SVG renderer bakes the vertical offset into `y`
        // rather than relying on `dominant-baseline`, which browsers handle
        // inconsistently.
        let baseline_offset = item
            .str("baseline")
            .map(|baseline| match baseline {
                "top" => 0.79 * font_size,
                "middle" => 0.30 * font_size,
                "bottom" => -0.21 * font_size,
                "line-top" => 0.29 * font_size + 0.5 * (font_size + 2.0),
                "line-bottom" => 0.29 * font_size - 0.5 * (font_size + 2.0),
                _ => 0.0,
            })
            .unwrap_or(0.0)
            .round();
        let angle = item.f64("angle").unwrap_or(0.0);
        // For rotated text the baseline shift happens in the rotated frame
        // (upstream emits `translate(x,y) rotate(a) translate(0,offset)`).
        if angle == 0.0 {
            y += baseline_offset;
        }

        let mut attrs = class_attr(item);
        push_num_attr(&mut attrs, "x", x);
        push_num_attr(&mut attrs, "y", y);

        if let Some(align) = item.str("align") {
            let anchor = match align {
                "left" | "start" => "start",
                "center" | "middle" => "middle",
                "right" | "end" => "end",
                _ => align,
            };
            push_str_attr(&mut attrs, "text-anchor", anchor);
        }
        if angle != 0.0 {
            let mut transform = format!(
                "rotate({} {} {})",
                fmt_num(angle),
                fmt_num(x),
                fmt_num(y)
            );
            if baseline_offset != 0.0 {
                transform.push_str(&format!(" translate(0,{})", fmt_num(baseline_offset)));
            }
            attrs.push_str(&format!(" transform=\"{}\"", transform));
        }

        if let Some(font) = item.str("font") {
            push_str_attr(&mut attrs, "font-family", font);
        }
        if let Some(v) = item.get("fontSize").and_then(value_string) {
            push_str_attr(&mut attrs, "font-size", &v);
        }
        if let Some(v) = item.get("fontWeight").and_then(value_string) {
            push_str_attr(&mut attrs, "font-weight", &v);
        }
        if let Some(v) = item.get("fontStyle").and_then(value_string) {
            push_str_attr(&mut attrs, "font-style", &v);
        }
        attrs.push_str(&self.style_attrs(item));

        let limit = item.f64("limit");
        let ellipsis = item.str("ellipsis").unwrap_or("\u{2026}");
        let line_height = item.f64("lineHeight").unwrap_or(font_size + 2.0);

        let content = match item.get("text") {
            Some(Value::Array(lines)) if lines.len() > 1 => {
                let mut s = String::new();
                for (i, line) in lines.iter().enumerate() {
                    let text = apply_limit(
                        scalar_text(line).trim(),
                        limit,
                        font_size,
                        ellipsis,
                    );
                    let dy = if i == 0 { 0.0 } else { line_height };
                    s.push_str(&format!(
                        "<tspan x=\"{}\" dy=\"{}\">{}</tspan>",
                        fmt_num(x),
                        fmt_num(dy),
                        escape_xml(&text)
                    ));
                }
                s
            }
            Some(other) => escape_xml(&apply_limit(
                scalar_text(other).trim(),
                limit,
                font_size,
                ellipsis,
            )),
            None => String::new(),
        };

        self.leaf("text", &attrs, Some(&content), item, true)
    }

    fn render_line(&mut self, item: &Item) -> String {
        let interpolate = item.str("interpolate").unwrap_or("linear");
        let d = line_path(&item.items, interpolate);

        let mut attrs = class_attr(item);
        push_str_attr(&mut attrs, "d", &d);
        if item.get("fill").map(|v| !v.is_null()).unwrap_or(false) {
            attrs.push_str(&self.style_attrs(item));
        } else {
            attrs.push_str(" fill=\"none\"");
            attrs.push_str(&self.style_attrs_no_fill(item));
        }

        if item.kind == "trail" && item.get("strokeWidth").is_none() {
            let width = item.f64("size").unwrap_or(2.0);
            push_num_attr(&mut attrs, "stroke-width", width);
        }

        self.leaf("path", &attrs, None, item, true)
    }

    fn render_trail(&mut self, item: &Item) -> String {
        let d = crate::scenegraph::trail_path(&item.items);
        let mut attrs = class_attr(item);
        push_str_attr(&mut attrs, "d", &d);
        attrs.push_str(&self.style_attrs(item));
        self.leaf("path", &attrs, None, item, true)
    }

    fn render_area(&mut self, item: &Item) -> String {
        let interpolate = item.str("interpolate").unwrap_or("linear");
        let orient = item.str("orient").unwrap_or("vertical");
        let d = area_path(&item.items, interpolate, orient);

        let mut attrs = class_attr(item);
        push_str_attr(&mut attrs, "d", &d);
        attrs.push_str(&self.style_attrs(item));
        self.leaf("path", &attrs, None, item, true)
    }

    fn render_arc(&mut self, item: &Item) -> String {
        let d = arc_path(item);
        let x = item.f64("x").unwrap_or(0.0);
        let y = item.f64("y").unwrap_or(0.0);
        let angle = item.f64("angle").unwrap_or(0.0);

        let mut attrs = class_attr(item);
        let mut transform = format!("translate({},{})", fmt_num(x), fmt_num(y));
        if angle != 0.0 {
            transform.push_str(&format!(" rotate({})", fmt_num(angle)));
        }
        push_str_attr(&mut attrs, "transform", &transform);
        push_str_attr(&mut attrs, "d", &d);
        attrs.push_str(&self.style_attrs(item));
        self.leaf("path", &attrs, None, item, true)
    }

    fn render_path(&mut self, item: &Item) -> String {
        let d = item.str("path").or_else(|| item.str("shape")).unwrap_or("");

        let x = item.f64("x").unwrap_or(0.0);
        let y = item.f64("y").unwrap_or(0.0);
        let mut transform = format!("translate({},{})", fmt_num(x), fmt_num(y));
        let angle = item.f64("angle").unwrap_or(0.0);
        if angle != 0.0 {
            transform.push_str(&format!(" rotate({})", fmt_num(angle)));
        }
        if let Some(scale) = item.get("scale") {
            match scale {
                Value::Array(a) => {
                    let parts: Vec<String> =
                        a.iter().filter(|v| !v.is_null()).map(attr_value).collect();
                    if !parts.is_empty() {
                        transform.push_str(&format!(" scale({})", parts.join(",")));
                    }
                }
                _ => {
                    if let Some(s) = value_string(scale) {
                        transform.push_str(&format!(" scale({})", s));
                    }
                }
            }
        }

        let mut attrs = class_attr(item);
        push_str_attr(&mut attrs, "transform", &transform);
        if !d.is_empty() {
            push_str_attr(&mut attrs, "d", d);
        }
        attrs.push_str(&self.style_attrs(item));
        self.leaf("path", &attrs, None, item, true)
    }

    fn render_image(&mut self, item: &Item) -> String {
        let href = item.str("href").or_else(|| item.str("url")).unwrap_or("");

        let w = item.f64("width").unwrap_or(0.0);
        let h = item.f64("height").unwrap_or(0.0);
        let mut x = item.f64("x").unwrap_or(0.0);
        let mut y = item.f64("y").unwrap_or(0.0);
        match item.str("align") {
            Some("center") => x -= w / 2.0,
            Some("right") | Some("end") => x -= w,
            _ => {}
        }
        match item.str("baseline") {
            Some("middle") => y -= h / 2.0,
            Some("bottom") => y -= h,
            _ => {}
        }
        let mut attrs = class_attr(item);
        push_num_attr(&mut attrs, "x", x);
        push_num_attr(&mut attrs, "y", y);
        push_num_attr(&mut attrs, "width", w);
        push_num_attr(&mut attrs, "height", h);
        push_str_attr(&mut attrs, "preserveAspectRatio", "xMidYMid");
        push_str_attr(&mut attrs, "href", href);
        push_str_attr(&mut attrs, "xlink:href", href);
        attrs.push_str(&self.style_attrs(item));
        // The image source itself is the link, so do not wrap the element.
        self.leaf("image", &attrs, None, item, false)
    }

    /// Build a leaf element, adding a `<title>` first child and the graphics
    /// role when the item has a description. Optionally wrap the element in an
    /// anchor when it has an `href`.
    fn leaf(
        &self,
        tag: &str,
        attrs: &str,
        content: Option<&str>,
        item: &Item,
        link: bool,
    ) -> String {
        let aria = aria_item_attrs(item, "");

        let mut out = String::new();
        out.push('<');
        out.push_str(tag);
        out.push_str(&aria);
        out.push_str(attrs);

        if content.is_none() {
            out.push_str("/>");
        } else {
            out.push('>');
            if let Some(content) = content {
                out.push_str(content);
            }
            out.push_str("</");
            out.push_str(tag);
            out.push('>');
        }

        if link {
            if let Some(href) = item.str("href") {
                return format!("<a href=\"{}\">{}</a>", escape_xml(href), out);
            }
        }
        out
    }

    fn style_attrs(&mut self, item: &Item) -> String {
        self.style_attrs_flags(item, true, true)
    }

    fn style_attrs_no_fill(&mut self, item: &Item) -> String {
        self.style_attrs_flags(item, false, true)
    }

    fn style_attrs_flags(&mut self, item: &Item, fill: bool, stroke: bool) -> String {
        let mut out = String::new();

        if fill {
            if let Some(paint) = item.get("fill").and_then(|v| self.paint(v)) {
                push_str_attr(&mut out, "fill", &paint);
            }
        }
        if let Some(v) = item.f64("fillOpacity") {
            push_num_attr(&mut out, "fill-opacity", v);
        }

        if stroke {
            if let Some(paint) = item.get("stroke").and_then(|v| self.paint(v)) {
                push_str_attr(&mut out, "stroke", &paint);
            }
        }
        if let Some(v) = item.f64("strokeOpacity") {
            push_num_attr(&mut out, "stroke-opacity", v);
        }
        if let Some(v) = item.f64("strokeWidth") {
            push_num_attr(&mut out, "stroke-width", v);
        }
        if let Some(v) = item.str("strokeCap") {
            push_str_attr(&mut out, "stroke-linecap", v);
        }
        if let Some(v) = item.str("strokeJoin") {
            push_str_attr(&mut out, "stroke-linejoin", v);
        }
        if let Some(dash) = item.get("strokeDash") {
            let value = match dash {
                Value::Array(a) => {
                    let parts: Vec<String> =
                        a.iter().filter(|v| !v.is_null()).map(attr_value).collect();
                    if parts.is_empty() {
                        None
                    } else {
                        Some(parts.join(","))
                    }
                }
                _ => value_string(dash),
            };
            if let Some(value) = value {
                push_str_attr(&mut out, "stroke-dasharray", &value);
            }
        }
        if let Some(v) = item.f64("strokeDashOffset") {
            push_num_attr(&mut out, "stroke-dashoffset", v);
        }
        if let Some(v) = item.f64("strokeMiterlimit") {
            push_num_attr(&mut out, "stroke-miterlimit", v);
        }
        if let Some(v) = item.f64("opacity") {
            push_num_attr(&mut out, "opacity", v);
        }
        if let Some(v) = item.str("blend") {
            push_str_attr(&mut out, "mix-blend-mode", v);
        }

        out
    }

    /// Resolve a paint value, emitting gradient definitions on the fly.
    fn paint(&mut self, value: &Value) -> Option<String> {
        if let Value::Object(obj) = value {
            if obj.contains_key("stops") || obj.contains_key("gradient") {
                return Some(self.gradient(obj));
            }
            return None;
        }
        value_string(value)
    }

    fn gradient(&mut self, obj: &Map<String, Value>) -> String {
        let id = format!("gradient_{}", self.gradient_count);
        self.gradient_count += 1;

        let kind = obj
            .get("gradient")
            .and_then(|v| v.as_str())
            .unwrap_or("linear");
        let radial = kind == "radial";

        let mut def = String::new();
        if radial {
            def.push_str(&format!("<radialGradient id=\"{}\"", id));
            for (from, to) in [
                ("x1", "cx"),
                ("y1", "cy"),
                ("r1", "r"),
                ("x2", "fx"),
                ("y2", "fy"),
                ("r2", "fr"),
            ] {
                if let Some(v) = obj.get(from).map(attr_value) {
                    push_str_attr(&mut def, to, &v);
                }
            }
        } else {
            def.push_str(&format!("<linearGradient id=\"{}\"", id));
            for (name, default) in [("x1", "0"), ("y1", "0"), ("x2", "0"), ("y2", "1")] {
                let value = obj
                    .get(name)
                    .map(attr_value)
                    .unwrap_or_else(|| default.to_string());
                push_str_attr(&mut def, name, &value);
            }
        }

        def.push('>');
        if let Some(Value::Array(stops)) = obj.get("stops") {
            for stop in stops {
                let mut attrs = String::new();
                let offset = stop
                    .get("offset")
                    .map(attr_value)
                    .unwrap_or_else(|| "0".to_string());
                push_str_attr(&mut attrs, "offset", &offset);
                if let Some(color) = stop.get("color").and_then(value_string) {
                    push_str_attr(&mut attrs, "stop-color", &color);
                }
                if let Some(opacity) = stop.get("opacity").map(attr_value) {
                    push_str_attr(&mut attrs, "stop-opacity", &opacity);
                }
                def.push_str("<stop");
                def.push_str(&attrs);
                def.push_str("/>");
            }
        }
        if radial {
            def.push_str("</radialGradient>");
        } else {
            def.push_str("</linearGradient>");
        }

        self.defs.push_str(&def);
        format!("url(#{})", id)
    }
}

fn has_class(item: &Item, class: &str) -> bool {
    item.class.iter().any(|c| c == class)
}

/// Mark type encoded in the item's `mark-<type>` class.
fn marktype_of(item: &Item) -> Option<&str> {
    item.class
        .iter()
        .find_map(|c| c.strip_prefix("mark-"))
}

/// Aria attributes for a mark container group (`<g class="mark-...">`),
/// mirroring `ariaMarkAttributes` in vega-scenegraph.
fn container_aria_attr(item: &Item) -> String {
    if has_class(item, "role-axis") {
        return " aria-hidden=\"true\"".to_string();
    }
    if has_class(item, "role-legend") {
        return " role=\"graphics-symbol\" aria-roledescription=\"legend\"".to_string();
    }
    if has_class(item, "role-title") {
        return String::new();
    }
    if item
        .class
        .iter()
        .any(|c| c.starts_with("role-axis-") || c.starts_with("role-legend-") || c.starts_with("role-title-"))
    {
        return String::new();
    }
    let Some(kind) = marktype_of(item) else {
        return String::new();
    };
    let marked = item
        .class
        .iter()
        .any(|c| c == "role-mark" || c == "role-frame" || c == "role-scope");
    let group_like = kind == "group";
    if !marked && !group_like {
        return String::new();
    }
    let object = group_like
        || kind == "text"
        || item
            .items
            .iter()
            .any(|child| child.str("description").is_some());
    let mut out = String::new();
    if let Some(desc) = item.str("description").filter(|s| !s.is_empty()) {
        push_str_attr(&mut out, "aria-label", desc);
    }
    push_str_attr(
        &mut out,
        "role",
        if object { "graphics-object" } else { "graphics-symbol" },
    );
    push_str_attr(&mut out, "aria-roledescription", &format!("{} mark container", kind));
    out
}

/// Aria attributes for a mark item (`<g transform=...>` group items and leaf
/// elements), mirroring `ariaItemAttributes` in vega-scenegraph.
fn item_aria_attr(item: &Item) -> String {
    aria_item_attrs(item, "")
}

fn aria_item_attrs(item: &Item, prefix: &str) -> String {
    if item.bool("aria") == Some(false) {
        return format!(" {}aria-hidden=\"true\"", prefix);
    }
    let Some(desc) = item.str("description").filter(|s| !s.is_empty()) else {
        return String::new();
    };
    let mut out = String::new();
    push_str_attr(&mut out, "aria-label", desc);
    let role = item
        .str("ariaRole")
        .unwrap_or(if item.kind == "group" {
            "graphics-object"
        } else {
            "graphics-symbol"
        });
    push_str_attr(&mut out, "role", role);
    let roledesc = item
        .str("ariaRoleDescription")
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("{} mark", item.kind));
    push_str_attr(&mut out, "aria-roledescription", &roledesc);
    out
}

fn is_group_mark(item: &Item) -> bool {
    has_class(item, "mark-group") && has_class(item, "role-mark")
}

/// Identity of the group mark a rendered group item belongs to. Runtime group
/// items are emitted per instance, so consecutive items sharing a class (and
/// optional mark name) are coalesced under one upstream `role-scope` container.
fn group_mark_key(item: &Item) -> (Vec<String>, Option<String>) {
    (
        item.class.clone(),
        item.str("_mark_name").map(|s| s.to_string()),
    )
}

fn is_guide_like(item: &Item) -> bool {
    item.class.iter().any(|c| {
        matches!(
            c.as_str(),
            "role-axis" | "role-frame" | "role-legend" | "role-title" | "role-scope"
        ) || c.starts_with("role-axis-")
            || c.starts_with("role-legend-")
    })
}

fn translate_attr(item: &Item) -> String {
    let nested_axis = item.class.iter().any(|c| c == "role-axis")
        && item
            .get("_nested_axis")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
    let extra = if nested_axis { 0.5 } else { 0.0 };
    let x = item.f64("x").unwrap_or(0.0) + item.f64("_render_dx").unwrap_or(0.0) + extra;
    let y = item.f64("y").unwrap_or(0.0) + item.f64("_render_dy").unwrap_or(0.0) + extra;
    format!(" transform=\"translate({},{})\"", fmt_num(x), fmt_num(y))
}

/// Mirror upstream group.js `offset()`: shift the group rectangle by half a
/// pixel when a thin stroke is used.
fn group_stroke_offset(item: &Item) -> f64 {
    if let Some(offset) = item.f64("strokeOffset") {
        return offset;
    }
    let sw = item.f64("strokeWidth").unwrap_or(1.0);
    let has_stroke = item
        .get("stroke")
        .map(is_visible_paint)
        .unwrap_or(false);
    if has_stroke && sw > 0.5 && sw < 1.5 {
        0.5 - (sw - 1.0).abs()
    } else {
        0.0
    }
}

fn class_attr(item: &Item) -> String {
    if item.class.is_empty() {
        String::new()
    } else {
        format!(" class=\"{}\"", escape_xml(&item.class.join(" ")))
    }
}

fn push_attr(out: &mut String, name: &str, value: &str) {
    out.push(' ');
    out.push_str(name);
    out.push_str("=\"");
    out.push_str(&escape_xml(value));
    out.push('"');
}

fn push_str_attr(out: &mut String, name: &str, value: &str) {
    push_attr(out, name, value);
}

fn push_num_attr(out: &mut String, name: &str, value: f64) {
    push_attr(out, name, &fmt_num(value));
}

fn value_string(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn attr_value(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.as_f64().map(fmt_num).unwrap_or_else(|| n.to_string()),
        Value::Bool(b) => b.to_string(),
        Value::Null => "0".to_string(),
        _ => String::new(),
    }
}

fn scalar_text(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => String::new(),
        Value::Array(a) => a.iter().map(scalar_text).collect::<Vec<_>>().join(","),
        Value::Object(_) => String::new(),
    }
}

fn is_visible_paint(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::String(s) => {
            let t = s.trim();
            !t.is_empty()
                && !t.eq_ignore_ascii_case("none")
                && !t.eq_ignore_ascii_case("transparent")
        }
        Value::Object(obj) => obj.contains_key("stops") || obj.contains_key("gradient"),
        _ => true,
    }
}

/// Upstream `textValue` truncation: only when `limit > 0`, using the estimated
/// text width `~~(0.8 * length * fontSize)` and the item's ellipsis.
fn apply_limit(text: &str, limit: Option<f64>, font_size: f64, ellipsis: &str) -> String {
    let Some(limit) = limit else {
        return text.to_string();
    };
    if limit <= 0.0 || font_size <= 0.0 || !limit.is_finite() {
        return text.to_string();
    }
    let chars: Vec<char> = text.chars().collect();
    let width = |s: &str| -> f64 { crate::bounds::text_width(s, font_size) };
    if width(text) < limit {
        return text.to_string();
    }
    let limit = limit - width(ellipsis);
    let mut lo = 0usize;
    let mut hi = chars.len();
    while lo < hi {
        let mid = 1 + ((lo + hi) >> 1);
        let prefix: String = chars[..mid].iter().collect();
        if width(&prefix) < limit {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    let mut out: String = chars[..lo].iter().collect();
    out.push_str(ellipsis);
    out
}

/// Escape all XML-sensitive characters in text and attribute values.
fn escape_xml(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn root(width: f64, height: f64) -> Item {
        let mut item = Item::new("group");
        item.class.push("mark-group".into());
        item.class.push("role-frame".into());
        item.class.push("root".into());
        item.set("width", json!(width));
        item.set("height", json!(height));
        item.set("_svg_width", json!(width));
        item.set("_svg_height", json!(height));
        item.set("_view_width", json!(width));
        item.set("_view_height", json!(height));
        item
    }

    #[test]
    fn simple_group_rect_text_and_escaping() {
        let mut root = root(200.0, 100.0);
        root.set("x", json!(5));
        root.set("y", json!(10));

        let mut group = Item::new("group");
        group.class.push("mark-rect".into());
        group.set("x", json!(2));
        group.set("y", json!(4));

        let mut rect = Item::new("rect");
        rect.set("x", json!(0));
        rect.set("y", json!(0));
        rect.set("width", json!(50));
        rect.set("height", json!(20));
        rect.set("fill", json!("#ff0000"));
        rect.set("strokeWidth", json!(0));
        group.items.push(rect);

        let mut text = Item::new("text");
        text.set("x", json!(10));
        text.set("y", json!(30));
        text.set("text", json!("A & B <x> \"q\" 'p'"));
        text.set("align", json!("center"));
        text.set("baseline", json!("middle"));
        group.items.push(text);

        root.items.push(group);
        let svg = render(&root, Some("white"));

        assert!(svg.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\""));
        assert!(svg.contains("width=\"200\" height=\"100\" viewBox=\"0 0 200 100\""));
        assert!(svg.contains("<rect width=\"200\" height=\"100\" fill=\"white\"/>"));
        assert!(svg.contains(
            "<g class=\"mark-group role-frame root\" role=\"graphics-object\" aria-roledescription=\"group mark container\" transform=\"translate(5,10)\">"
        ));
        assert!(svg.contains("<g class=\"mark-rect\" transform=\"translate(2,4)\">"));
        assert!(svg.contains("<path d=\"M0,0h50v20h-50Z\" fill=\"#ff0000\" stroke-width=\"0\"/>"));
        assert!(svg.contains("text-anchor=\"middle\""));
        // vega-scenegraph bakes the baseline offset into y (0.30 * 11 -> 3).
        assert!(svg.contains("x=\"10\" y=\"33\""));
        assert!(!svg.contains("dominant-baseline"));
        assert!(svg.contains("A &amp; B &lt;x&gt; &quot;q&quot; &apos;p&apos;"));
        assert!(!svg.contains("A & B"));
    }

    #[test]
    fn gradient_defs_are_emitted_before_groups() {
        let mut root = root(100.0, 50.0);
        let mut rect = Item::new("rect");
        rect.set("x", json!(0));
        rect.set("y", json!(0));
        rect.set("width", json!(100));
        rect.set("height", json!(50));
        rect.set(
            "fill",
            json!({
                "gradient": "linear",
                "x1": 0, "y1": 0, "x2": 0, "y2": 1,
                "stops": [
                    {"offset": 0, "color": "#ffffff"},
                    {"offset": 1, "color": "#000000"}
                ]
            }),
        );
        root.items.push(rect);

        let svg = render(&root, None);
        assert!(
            svg.contains("<linearGradient id=\"gradient_0\" x1=\"0\" y1=\"0\" x2=\"0\" y2=\"1\">")
        );
        assert!(svg.contains("<stop offset=\"0\" stop-color=\"#ffffff\"/>"));
        assert!(svg.contains("<stop offset=\"1\" stop-color=\"#000000\"/>"));
        assert!(svg.contains("fill=\"url(#gradient_0)\""));
        let defs = svg.find("<defs>").unwrap();
        let group = svg.find("<g fill=\"none\"").unwrap();
        assert!(defs < group);
    }

    #[test]
    fn line_path_and_trail_width() {
        let mut root = root(100.0, 100.0);
        let mut line = Item::new("line");
        line.set("stroke", json!("#00f"));
        line.set("strokeWidth", json!(3));
        for (x, y) in [(0.0, 0.0), (10.0, 10.0), (20.0, 5.0)] {
            let mut v = Item::new("vertex");
            v.set("x", json!(x));
            v.set("y", json!(y));
            line.items.push(v);
        }
        root.items.push(line);

        let mut trail = Item::new("trail");
        trail.set("fill", json!("#f00"));
        trail.set("size", json!(4));
        for (x, y) in [(0.0, 0.0), (5.0, 5.0)] {
            let mut v = Item::new("vertex");
            v.set("x", json!(x));
            v.set("y", json!(y));
            trail.items.push(v);
        }
        root.items.push(trail);

        let svg = render(&root, None);
        assert!(svg.contains(
            "<path d=\"M0,0L10,10L20,5\" fill=\"none\" stroke=\"#00f\" stroke-width=\"3\"/>"
        ));
        assert!(svg.contains("fill=\"#f00\""));
        assert!(svg.contains("A "));
    }

    #[test]
    fn text_array_tspans_and_limit() {
        let mut root = root(100.0, 100.0);
        let mut text = Item::new("text");
        text.set("x", json!(10));
        text.set("y", json!(20));
        text.set("fontSize", json!(10));
        text.set("lineHeight", json!(12));
        text.set("text", json!(["first", "second"]));
        root.items.push(text);

        let mut truncated = Item::new("text");
        truncated.set("x", json!(0));
        truncated.set("y", json!(0));
        truncated.set("fontSize", json!(10));
        truncated.set("limit", json!(28));
        truncated.set("text", json!("abcdefghij"));
        root.items.push(truncated);

        let svg = render(&root, None);
        assert!(svg.contains("<tspan x=\"10\" dy=\"0\">first</tspan>"));
        assert!(svg.contains("<tspan x=\"10\" dy=\"12\">second</tspan>"));
        assert!(svg.contains(">abc…</text>"));
    }

    fn path_count(svg: &str) -> usize {
        svg.matches("<path").count()
    }

    fn g_count(svg: &str) -> usize {
        svg.matches("<g").count()
    }

    #[test]
    fn legend_entry_uses_upstream_scope_and_mark_wrappers() {
        let mut root = root(100.0, 100.0);

        let mut legend = Item::new("group");
        legend.class.push("mark-group".into());
        legend.class.push("role-legend".into());
        legend.set("width", json!(40));
        legend.set("height", json!(40));

        let mut title = Item::new("text");
        title.set("text", json!("Legend"));
        legend.items.push(title);

        for (i, label) in ["a", "b"].iter().enumerate() {
            let mut entry = Item::new("group");
            entry.class.push("role-legend-entry".into());
            entry.set("x", json!(0));
            entry.set("y", json!(i as f64 * 20.0));

            let mut symbol = Item::new("symbol");
            symbol.set("x", json!(8));
            symbol.set("y", json!(i as f64 * 20.0 + 8.0));
            symbol.set("size", json!(64));
            symbol.set("fill", json!("#4c78a8"));
            entry.items.push(symbol);

            let mut text = Item::new("text");
            text.set("x", json!(20));
            text.set("y", json!(i as f64 * 20.0 + 8.0));
            text.set("text", json!(label));
            entry.items.push(text);

            legend.items.push(entry);
        }
        root.items.push(legend);

        let svg = render(&root, None);
        assert!(svg.contains("<g class=\"mark-group role-legend\">"));
        assert!(svg.contains("<g class=\"mark-group role-legend-entry\">"));
        assert!(svg.contains("<g class=\"mark-group role-scope\">"));
        assert_eq!(svg.matches("mark-symbol role-legend-symbol").count(), 2);
        assert_eq!(svg.matches("mark-text role-legend-label").count(), 2);
        assert!(svg.contains("<g class=\"mark-text role-legend-title\">"));
        // container + item + content + entry container + entry item +
        // entry content + scope container + 2 * (item + content + symbol +
        // label) + title
        assert_eq!(g_count(&svg), 18);
        // root bg/fg, legend bg/fg, entry bg/fg, 2 * scope bg/fg and 2 symbols
        assert_eq!(path_count(&svg), 12);
    }

    #[test]
    fn faceted_group_mark_instances_share_one_scope_container() {
        let mut root = root(100.0, 100.0);

        for i in 0..3 {
            let mut group = Item::new("group");
            group.class.push("mark-group".into());
            group.class.push("role-mark".into());
            group.set("_mark_name", json!("cell"));
            group.set("x", json!(i * 10));
            group.set("y", json!(0));
            group.set("width", json!(10));
            group.set("height", json!(10));

            let mut child = Item::new("group");
            child.class.push("mark-rect".into());
            child.class.push("role-mark".into());
            let mut rect = Item::new("rect");
            rect.set("width", json!(5));
            rect.set("height", json!(5));
            rect.set("fill", json!("#4c78a8"));
            child.items.push(rect);
            group.items.push(child);
            root.items.push(group);
        }

        let svg = render(&root, None);
        assert_eq!(svg.matches("role-scope cell").count(), 1);
        assert_eq!(g_count(&svg), 12);
        // root bg/fg + 3 group item bg/fg + 3 rect marks
        assert_eq!(path_count(&svg), 2 + 3 * 2 + 3);
    }
}
