use serde_json::Value;

pub fn render_kp_zone_svg_from_value(val: &Value) -> String {
    let z1 = val
        .get("zone1_depth_m")
        .and_then(|v| v.as_f64())
        .unwrap_or(6.0);
    let z2 = val
        .get("zone2_depth_m")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let z3 = val
        .get("zone3_depth_m")
        .and_then(|v| v.as_f64())
        .filter(|v| *v > 0.0);
    let category = val
        .get("category")
        .and_then(|v| v.as_str())
        .unwrap_or("private-office");
    let area_m2 = val.get("area_m2").and_then(|v| v.as_f64());
    render_kp_zone_svg(z1, z2, z3, category, area_m2)
}

#[allow(dead_code)]
pub fn render_kp_fraction_svg(display_name: &str) -> String {
    let fraction = if display_name.contains("1/8") {
        0.125
    } else if display_name.contains("1/4") {
        0.25
    } else if display_name.contains("1/3") {
        1.0 / 3.0
    } else if display_name.contains("1/2") {
        0.5
    } else {
        1.0
    };
    let fill_w = (164.0 * fraction) as u32;
    let label = if display_name.contains("1/8") {
        "1/8 Floor"
    } else if display_name.contains("1/4") {
        "1/4 Floor"
    } else if display_name.contains("1/3") {
        "1/3 Floor"
    } else if display_name.contains("1/2") {
        "1/2 Floor"
    } else {
        "Full Floor"
    };
    format!(
        "<svg class=\"bim-kp-diagram\" viewBox=\"0 0 180 112\" xmlns=\"http://www.w3.org/2000/svg\" aria-hidden=\"true\">\
<text x=\"90\" y=\"8\" font-size=\"7\" fill=\"#888\" font-family=\"sans-serif\" text-anchor=\"middle\" letter-spacing=\"1.5\">FLOOR PLATE</text>\
<rect x=\"8\" y=\"12\" width=\"164\" height=\"88\" fill=\"#ebebeb\" stroke=\"#ccc\" stroke-width=\"0.5\"/>\
<rect x=\"8\" y=\"12\" width=\"{fw}\" height=\"88\" fill=\"#c8d8e8\" stroke=\"#a0b8cc\" stroke-width=\"0.5\"/>\
<text x=\"90\" y=\"62\" font-size=\"14\" fill=\"#5a7898\" font-family=\"sans-serif\" text-anchor=\"middle\" font-weight=\"600\">{lbl}</text>\
<text x=\"90\" y=\"80\" font-size=\"8\" fill=\"#888\" font-family=\"sans-serif\" text-anchor=\"middle\">of net leasable area</text>\
<text x=\"90\" y=\"110\" font-size=\"7\" fill=\"#888\" font-family=\"sans-serif\" text-anchor=\"middle\" letter-spacing=\"1.5\">SIZED AGAINST FLOOR PLATE</text>\
</svg>",
        fw = fill_w,
        lbl = label,
    )
}

pub fn render_kp_zone_svg(
    z1: f64,
    z2: f64,
    z3: Option<f64>,
    category: &str,
    area_m2: Option<f64>,
) -> String {
    let d3 = z3.unwrap_or(0.0);
    let total = z1 + z2 + d3;
    if total <= 0.0 {
        return String::new();
    }

    let accent = match category {
        "private-office" => "#1a3a5c",
        "medical" => "#7a1a1a",
        "laboratory" => "#1a4060",
        "business" => "#7a4a00",
        "academic" => "#4a4800",
        "civic" => "#1a5430",
        _ => "#303040",
    };

    // Drawing area: x=22, y=10, max_w=153, h=94 within 180×112 viewBox.
    // Left 22px reserved for Z1/Z2/Z3 labels.
    let x0: f64 = 22.0;
    let y0: f64 = 10.0;
    let max_dw: f64 = 153.0;
    let dh: f64 = 94.0;

    // Proportional width: frontage = area / depth. Normalise against 6 m reference.
    let frontage = area_m2.map(|a| a / total).unwrap_or(total);
    let plan_w = ((frontage / 6.0) * max_dw).clamp(max_dw * 0.30, max_dw);
    let xr: f64 = x0 + plan_w;

    let size_tier: u8 = match (category, area_m2) {
        ("private-office", Some(a)) => {
            if a < 38.0 {
                0
            } else if a < 55.0 {
                1
            } else {
                2
            }
        }
        ("medical", Some(a)) => {
            if a < 270.0 {
                0
            } else if a < 410.0 {
                1
            } else {
                2
            }
        }
        ("laboratory", Some(a)) => {
            if a < 260.0 {
                0
            } else if a < 370.0 {
                1
            } else {
                2
            }
        }
        ("academic", Some(a)) => {
            if a < 175.0 {
                0
            } else if a < 315.0 {
                1
            } else {
                2
            }
        }
        ("business", Some(a)) => {
            if a < 360.0 {
                0
            } else if a < 545.0 {
                1
            } else {
                2
            }
        }
        ("civic", Some(a)) => {
            if a < 420.0 {
                0
            } else if a < 700.0 {
                1
            } else {
                2
            }
        }
        _ => 1,
    };

    let h1 = (z1 / total) * dh;
    let h2 = (z2 / total) * dh;
    let h3 = dh - h1 - h2;
    let y1 = y0 + h1;
    let y2 = y1 + h2;

    let lz1a = y0 + h1 * 0.38;
    let lz1b = y0 + h1 * 0.64;
    let lz2a = y1 + h2 * 0.38;
    let lz2b = y1 + h2 * 0.64;
    let lz3a = y2 + h3 * 0.38;
    let lz3b = y2 + h3 * 0.68;

    let mut s = String::with_capacity(2400);

    s.push_str("<svg class=\"bim-kp-diagram\" viewBox=\"0 0 180 112\" xmlns=\"http://www.w3.org/2000/svg\" aria-hidden=\"true\">");
    s.push_str("<rect width=\"180\" height=\"112\" fill=\"#f0f4f8\"/>");

    s.push_str(&format!(
        "<text x=\"108\" y=\"8.5\" font-size=\"5.5\" fill=\"{}\" font-family=\"sans-serif\" text-anchor=\"middle\" letter-spacing=\"1.2\">FACADE</text>",
        accent
    ));

    // Mullion ticks (4 evenly spaced along facade edge)
    let mull_step = plan_w / 5.0;
    for i in 1u8..=4 {
        let mx = x0 + mull_step * i as f64;
        s.push_str(&format!(
            "<line x1=\"{:.1}\" y1=\"6\" x2=\"{:.1}\" y2=\"{:.0}\" stroke=\"{}\" stroke-width=\"0.8\"/>",
            mx, mx, y0, accent
        ));
    }

    // Zone fills (blueprint paper tints)
    s.push_str(&format!(
        "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{:.1}\" fill=\"#fff9f4\"/>",
        x0, y0, plan_w, h1
    ));
    s.push_str(&format!(
        "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{:.1}\" fill=\"#fafae8\"/>",
        x0, y1, plan_w, h2
    ));
    if h3 >= 1.0 {
        s.push_str(&format!(
            "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{:.1}\" fill=\"#f0f8f2\"/>",
            x0, y2, plan_w, h3
        ));
    }

    // Perimeter (accent colour)
    s.push_str(&format!(
        "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{:.1}\" fill=\"none\" stroke=\"{}\" stroke-width=\"1.2\"/>",
        x0, y0, plan_w, dh, accent
    ));

    // Zone boundary dashed lines
    s.push_str(&format!(
        "<line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" stroke=\"#8a9aaa\" stroke-dasharray=\"3.5,2.5\" stroke-width=\"0.75\"/>",
        x0, y1, xr, y1
    ));
    if h3 >= 1.0 {
        s.push_str(&format!(
            "<line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" stroke=\"#8a9aaa\" stroke-dasharray=\"3.5,2.5\" stroke-width=\"0.75\"/>",
            x0, y2, xr, y2
        ));
    }

    // Zone labels (left of plan)
    s.push_str(&format!(
        "<text x=\"21\" y=\"{:.1}\" font-size=\"5\" fill=\"#6a4820\" font-family=\"sans-serif\" text-anchor=\"end\">Z1</text>",
        lz1a
    ));
    s.push_str(&format!(
        "<text x=\"21\" y=\"{:.1}\" font-size=\"4\" fill=\"#8a6840\" font-family=\"sans-serif\" text-anchor=\"end\">{:.1}m</text>",
        lz1b, z1
    ));
    s.push_str(&format!(
        "<text x=\"21\" y=\"{:.1}\" font-size=\"5\" fill=\"#4a5020\" font-family=\"sans-serif\" text-anchor=\"end\">Z2</text>",
        lz2a
    ));
    s.push_str(&format!(
        "<text x=\"21\" y=\"{:.1}\" font-size=\"4\" fill=\"#6a7040\" font-family=\"sans-serif\" text-anchor=\"end\">{:.1}m</text>",
        lz2b, z2
    ));
    if h3 >= 8.0 {
        s.push_str(&format!(
            "<text x=\"21\" y=\"{:.1}\" font-size=\"5\" fill=\"#205040\" font-family=\"sans-serif\" text-anchor=\"end\">Z3</text>",
            lz3a
        ));
        if h3 >= 14.0 {
            s.push_str(&format!(
                "<text x=\"21\" y=\"{:.1}\" font-size=\"4\" fill=\"#407060\" font-family=\"sans-serif\" text-anchor=\"end\">{:.1}m</text>",
                lz3b, d3
            ));
        }
    }

    // ── Furniture macros ───────────────────────────────────────────────────────
    macro_rules! desk {
        ($s:expr, $dx:expr, $dy:expr) => {
            $s.push_str(&format!(
                "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"15\" height=\"9\" fill=\"#c8b48a\" stroke=\"#8b6a40\" stroke-width=\"0.5\" rx=\"0.5\"/>",
                $dx, $dy
            ));
            $s.push_str(&format!(
                "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"3\" fill=\"#b0a080\" stroke=\"#8b6a40\" stroke-width=\"0.4\"/>",
                ($dx as f64) + 7.5,
                ($dy as f64) + 13.0
            ));
        };
    }
    macro_rules! round_table {
        ($s:expr, $cx:expr, $cy:expr, $r:expr, $n:expr) => {{
            let (cx, cy, r) = ($cx as f64, $cy as f64, $r as f64);
            $s.push_str(&format!(
                "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"{:.1}\" fill=\"#d4c4a0\" stroke=\"#8b6a40\" stroke-width=\"0.5\"/>",
                cx, cy, r
            ));
            let offsets: &[(f64, f64)] =
                &[(0.0, -(r + 3.5)), (r + 3.5, 0.0), (0.0, r + 3.5), (-(r + 3.5), 0.0)];
            for &(dx, dy) in offsets.iter().take($n) {
                $s.push_str(&format!(
                    "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"2.5\" fill=\"#b0a080\" stroke=\"#8b6a40\" stroke-width=\"0.4\"/>",
                    cx + dx,
                    cy + dy
                ));
            }
        }};
    }
    macro_rules! door {
        ($s:expr, $dx:expr, $dy:expr, $dh:expr) => {{
            let (dx, dy, dh) = ($dx as f64, $dy as f64, $dh as f64);
            $s.push_str(&format!(
                "<line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" stroke=\"#556677\" stroke-width=\"0.75\"/>",
                dx, dy, dx, dy + dh
            ));
            $s.push_str(&format!(
                "<path d=\"M{:.1},{:.1} A{:.1},{:.1} 0 0,1 {:.1},{:.1}\" stroke=\"#556677\" stroke-width=\"0.75\" fill=\"none\" stroke-dasharray=\"2,1.5\"/>",
                dx, dy, dh, dh, dx + dh * 0.87, dy + dh * 0.5
            ));
        }};
    }

    match category {
        // ── Private Office ─────────────────────────────────────────────────
        "private-office" => {
            let desk_n = size_tier as usize + 1;
            for i in 0..desk_n {
                desk!(s, x0 + 3.0 + 19.0 * i as f64, y0 + 3.0);
            }
            if h1 >= 25.0 {
                let tbl_r = (h1 * 0.18).clamp(7.0, 10.0);
                let tbl_x = (x0 + plan_w * 0.58).min(xr - tbl_r - 12.0);
                round_table!(s, tbl_x, y0 + h1 * 0.72, tbl_r, 3);
            }
            let cred_x = (xr - 17.0).max(x0 + 3.0 + 19.0 * desk_n as f64 + 3.0);
            s.push_str(&format!(
                "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"14\" height=\"5\" fill=\"#c8b48a\" stroke=\"#8b6a40\" stroke-width=\"0.4\" rx=\"0.3\"/>",
                cred_x, y0 + 3.0
            ));
            if h2 >= 10.0 {
                let cw = (plan_w * 0.65).min(85.0);
                s.push_str(&format!(
                    "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"5\" fill=\"#c8b48a\" stroke=\"#8b6a40\" stroke-width=\"0.5\" rx=\"0.3\"/>",
                    x0 + 3.0, y1 + 3.0, cw
                ));
                s.push_str(&format!(
                    "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"9\" height=\"9\" fill=\"#b8c8d8\" stroke=\"#5a7898\" stroke-width=\"0.4\" rx=\"0.3\"/>",
                    (x0 + cw + 5.0).min(xr - 12.0), y1 + 2.0
                ));
            }
            if h3 >= 10.0 {
                door!(s, x0 + 4.0, y2, (h3 * 0.85).min(13.0));
            }
        }

        // ── Medical ────────────────────────────────────────────────────────
        "medical" => {
            let doc_n = if size_tier == 2 { 2usize } else { 1 };
            let chair_n = match size_tier {
                0 => 2usize,
                1 => 4,
                _ => 6,
            };
            for i in 0..doc_n {
                let ox = x0 + 1.0 + 21.0 * i as f64;
                s.push_str(&format!(
                    "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"19\" height=\"{:.1}\" fill=\"#e8e0d0\" stroke=\"{}\" stroke-width=\"0.6\"/>",
                    ox, y0, h1, accent
                ));
                s.push_str(&format!(
                    "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"10\" height=\"6\" fill=\"#c8b48a\" stroke=\"#8b6a40\" stroke-width=\"0.4\" rx=\"0.3\"/>",
                    ox + 2.0, y0 + h1 - 9.0
                ));
            }
            let ch_x0 = x0 + 2.0 + 21.0 * doc_n as f64;
            let ch_area = (xr - 28.0) - ch_x0;
            if h1 >= 15.0 && ch_area > 0.0 {
                let sp = (ch_area / chair_n as f64).max(11.0);
                let cy = y0 + h1 * 0.35;
                for i in 0..chair_n {
                    let cx = ch_x0 + sp * i as f64;
                    if cx + 10.0 > xr - 28.0 {
                        break;
                    }
                    s.push_str(&format!(
                        "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"10\" height=\"7\" fill=\"#d0e4d0\" stroke=\"#5a8a6a\" stroke-width=\"0.5\" rx=\"1.5\"/>",
                        cx, cy
                    ));
                    s.push_str(&format!(
                        "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"3.5\" fill=\"#e4f0e4\" stroke=\"#5a8a6a\" stroke-width=\"0.4\"/>",
                        cx + 5.0, cy - 4.0
                    ));
                }
            }
            s.push_str(&format!(
                "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"22\" height=\"7\" fill=\"#c8b48a\" stroke=\"#8b6a40\" stroke-width=\"0.5\" rx=\"0.5\"/>",
                xr - 26.0, y0 + 4.0
            ));
            if h2 >= 10.0 {
                let bw = (plan_w * 0.60).min(100.0);
                s.push_str(&format!(
                    "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"6\" fill=\"#c0d8c0\" stroke=\"#5a8a6a\" stroke-width=\"0.5\" rx=\"0.5\"/>",
                    x0 + 4.0, y1 + 3.0, bw
                ));
                let sects = (bw / 18.0) as usize;
                for i in 1..sects {
                    let bx = x0 + 4.0 + 18.0 * i as f64;
                    s.push_str(&format!(
                        "<line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" stroke=\"#5a8a6a\" stroke-width=\"0.3\"/>",
                        bx, y1 + 3.0, bx, y1 + 9.0
                    ));
                }
                s.push_str(&format!(
                    "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"11\" height=\"11\" fill=\"#c8d8e0\" stroke=\"#5a7898\" stroke-width=\"0.5\" rx=\"1\"/>",
                    xr - 15.0, y1 + 2.0
                ));
            }
            if h3 >= 8.0 {
                s.push_str(&format!(
                    "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"16\" height=\"{:.1}\" fill=\"#e8f0f8\" stroke=\"#7090a8\" stroke-width=\"0.5\" rx=\"0.5\"/>",
                    xr - 20.0, y2 + 2.0, (h3 - 4.0).max(5.0)
                ));
            }
        }

        // ── Laboratory ─────────────────────────────────────────────────────
        "laboratory" => {
            let office_n = if size_tier == 0 { 1usize } else { 2 };
            let bench_n = match size_tier {
                0 => 3usize,
                1 => 5,
                _ => 7,
            };
            let rec_h = (h1 * 0.55).max(15.0).min(h1);
            s.push_str(&format!(
                "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"16\" height=\"{:.1}\" fill=\"#e8e0d0\" stroke=\"{}\" stroke-width=\"0.5\"/>",
                x0 + 1.0, y0, rec_h, accent
            ));
            for i in 0..office_n {
                let ox = x0 + 19.0 + 20.0 * i as f64;
                let off_h = (h1 * 0.65).min(h1);
                s.push_str(&format!(
                    "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"18\" height=\"{:.1}\" fill=\"#e8e0d0\" stroke=\"{}\" stroke-width=\"0.5\"/>",
                    ox, y0, off_h, accent
                ));
                if off_h >= 16.0 {
                    desk!(s, ox + 2.0, y0 + off_h - 13.0);
                }
            }
            let bx0 = x0 + 20.0 + 20.0 * office_n as f64;
            let b_area = xr - 4.0 - bx0;
            if b_area > 0.0 && h1 >= 12.0 {
                let bs = b_area / bench_n as f64;
                for i in 0..bench_n {
                    let bx = bx0 + bs * i as f64;
                    if bx + 11.0 > xr - 2.0 {
                        break;
                    }
                    s.push_str(&format!(
                        "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"11\" height=\"6\" fill=\"#c0d0c8\" stroke=\"#5a8a6a\" stroke-width=\"0.5\" rx=\"0.3\"/>",
                        bx, y0 + 4.0
                    ));
                    s.push_str(&format!(
                        "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"2.5\" fill=\"#a0a8b0\" stroke=\"#607080\" stroke-width=\"0.4\"/>",
                        bx + 5.5, y0 + 14.0
                    ));
                }
            }
            if h2 >= 10.0 {
                let sr_w = 30.0f64;
                s.push_str(&format!(
                    "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{:.1}\" fill=\"#e0e8e0\" stroke=\"{}\" stroke-width=\"0.5\"/>",
                    x0 + 1.0, y1, sr_w, h2 * 0.85, accent
                ));
                round_table!(s, x0 + 1.0 + sr_w / 2.0, y1 + h2 * 0.42, 6.0, 4);
                let sb_w = (plan_w - sr_w - 10.0).clamp(0.0, 100.0);
                if sb_w > 0.0 {
                    s.push_str(&format!(
                        "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"6\" fill=\"#c8d8c0\" stroke=\"#5a8a6a\" stroke-width=\"0.4\" rx=\"0.3\"/>",
                        x0 + sr_w + 5.0, y1 + 3.0, sb_w
                    ));
                }
            }
            if h3 >= 8.0 {
                s.push_str(&format!(
                    "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"32\" height=\"5\" fill=\"#d0e0d8\" stroke=\"#5a8a6a\" stroke-width=\"0.5\" rx=\"0.3\"/>",
                    x0 + 4.0, y2 + 2.0
                ));
            }
        }

        // ── Business ───────────────────────────────────────────────────────
        "business" => {
            let office_n: usize = match size_tier {
                0 => 2,
                1 => 3,
                _ => 5,
            };
            let ws_cols: usize = match size_tier {
                0 => 3,
                1 => 4,
                _ => 5,
            };
            let ws_rows: usize = match size_tier {
                0 => 3,
                1 => 4,
                _ => 5,
            };
            let conf_n: usize = if size_tier == 2 { 2 } else { 1 };
            let col_n = if office_n > 3 { 2usize } else { 1 };
            let per_col = office_n.div_ceil(col_n);
            let oh = ((h1 - 6.0) / per_col as f64).min(13.0);
            s.push_str(&format!(
                "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"3\" fill=\"#c8b48a\" stroke=\"#8b6a40\" stroke-width=\"0.3\" rx=\"0.3\"/>",
                x0 + 1.0, y0, 16.0 * col_n as f64 + 2.0 * (col_n - 1) as f64
            ));
            for i in 0..per_col {
                let oy = y0 + 5.0 + oh * i as f64;
                if oy + oh > y0 + h1 - 1.0 {
                    break;
                }
                s.push_str(&format!(
                    "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"16\" height=\"{:.1}\" fill=\"#e8e0d0\" stroke=\"{}\" stroke-width=\"0.5\"/>",
                    x0 + 1.0, oy, oh - 0.5, accent
                ));
            }
            if col_n == 2 {
                for i in 0..(office_n - per_col) {
                    let oy = y0 + 5.0 + oh * i as f64;
                    if oy + oh > y0 + h1 - 1.0 {
                        break;
                    }
                    s.push_str(&format!(
                        "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"16\" height=\"{:.1}\" fill=\"#e8e0d0\" stroke=\"{}\" stroke-width=\"0.5\"/>",
                        x0 + 19.0, oy, oh - 0.5, accent
                    ));
                }
            }
            let ws_x0 = x0 + 2.0 + 18.0 * col_n as f64;
            let ws_aw = xr - ws_x0 - 3.0;
            let ws_sx = ws_aw / ws_cols as f64;
            let ws_sy = (h1 - 2.0) / ws_rows as f64;
            for row in 0..ws_rows {
                for col in 0..ws_cols {
                    let wx = ws_x0 + ws_sx * col as f64;
                    let wy = y0 + 1.0 + ws_sy * row as f64;
                    let ww = (ws_sx - 2.0).clamp(5.0, 16.0);
                    let wh = (ws_sy - 1.5).clamp(3.0, 10.0);
                    if wx + ww > xr - 2.0 {
                        break;
                    }
                    s.push_str(&format!(
                        "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{:.1}\" fill=\"#d0c8e0\" stroke=\"#7060a0\" stroke-width=\"0.4\" rx=\"0.3\"/>",
                        wx, wy, ww, wh
                    ));
                }
            }
            if h2 >= 12.0 {
                let cw = (plan_w * 0.38).min(58.0);
                let ch = (h2 * 0.48).clamp(8.0, 16.0);
                for ci in 0..conf_n {
                    let cx_t = x0 + 4.0 + ci as f64 * (cw + 6.0);
                    if cx_t + cw > xr - 26.0 {
                        break;
                    }
                    let cy_t = y1 + (h2 - ch) / 2.0;
                    s.push_str(&format!(
                        "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{:.1}\" fill=\"#d4c8a0\" stroke=\"#8b7a40\" stroke-width=\"0.5\" rx=\"1\"/>",
                        cx_t, cy_t, cw, ch
                    ));
                    let cc = ((cw / 10.0) as usize).max(2);
                    for j in 0..cc {
                        let chair_x = cx_t + (cw / cc as f64) * (j as f64 + 0.5) - 3.0;
                        s.push_str(&format!(
                            "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"6\" height=\"3.5\" fill=\"#b0a880\" stroke=\"#8b7a40\" stroke-width=\"0.3\" rx=\"0.5\"/>",
                            chair_x, cy_t - 4.5
                        ));
                        s.push_str(&format!(
                            "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"6\" height=\"3.5\" fill=\"#b0a880\" stroke=\"#8b7a40\" stroke-width=\"0.3\" rx=\"0.5\"/>",
                            chair_x, cy_t + ch + 1.0
                        ));
                    }
                }
                s.push_str(&format!(
                    "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"22\" height=\"{:.1}\" fill=\"#e0e8e0\" stroke=\"{}\" stroke-width=\"0.4\"/>",
                    xr - 24.0, y1, h2 * 0.75, accent
                ));
            }
            if h3 >= 8.0 {
                s.push_str(&format!(
                    "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"16\" height=\"{:.1}\" fill=\"#e8f0f8\" stroke=\"#7090a8\" stroke-width=\"0.5\" rx=\"0.3\"/>",
                    xr - 20.0, y2 + 1.0, (h3 - 3.0).max(4.0)
                ));
            }
        }

        // ── Academic ───────────────────────────────────────────────────────
        "academic" => {
            match size_tier {
                0 => {
                    for row in 0..4usize {
                        for col in 0..2usize {
                            s.push_str(&format!(
                                "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"10\" height=\"7\" fill=\"#d0c8e0\" stroke=\"#7060a0\" stroke-width=\"0.4\" rx=\"0.3\"/>",
                                x0 + 3.0 + col as f64 * 12.0, y0 + 5.0 + row as f64 * 12.0
                            ));
                        }
                    }
                    let ctw = 42.0f64;
                    let cth = (h1 * 0.45).clamp(14.0, 22.0);
                    let cty = y0 + (h1 - cth) / 2.0;
                    s.push_str(&format!(
                        "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{:.1}\" fill=\"#d4c8a0\" stroke=\"#8b7a40\" stroke-width=\"0.5\" rx=\"1\"/>",
                        x0 + 29.0, cty, ctw, cth
                    ));
                    round_table!(s, x0 + 86.0, y0 + h1 * 0.28, 8.0, 4);
                    round_table!(s, x0 + 86.0, y0 + h1 * 0.72, 8.0, 4);
                }
                1 => {
                    for row in 0..4usize {
                        for col in 0..2usize {
                            s.push_str(&format!(
                                "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"10\" height=\"7\" fill=\"#d0c8e0\" stroke=\"#7060a0\" stroke-width=\"0.4\" rx=\"0.3\"/>",
                                x0 + 3.0 + col as f64 * 12.0, y0 + 5.0 + row as f64 * 12.0
                            ));
                            let rx2 = xr - 25.0 + col as f64 * 12.0;
                            if rx2 + 10.0 < xr - 2.0 {
                                s.push_str(&format!(
                                    "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"10\" height=\"7\" fill=\"#d0c8e0\" stroke=\"#7060a0\" stroke-width=\"0.4\" rx=\"0.3\"/>",
                                    rx2, y0 + 5.0 + row as f64 * 12.0
                                ));
                            }
                        }
                    }
                    let ctw = 58.0f64;
                    let cth = (h1 * 0.50).clamp(18.0, 26.0);
                    let cty = y0 + (h1 - cth) / 2.0;
                    s.push_str(&format!(
                        "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{:.1}\" fill=\"#d4c8a0\" stroke=\"#8b7a40\" stroke-width=\"0.5\" rx=\"{:.1}\"/>",
                        x0 + 29.0, cty, ctw, cth, cth / 2.0
                    ));
                    round_table!(s, x0 + 104.0, y0 + h1 * 0.5, 8.0, 4);
                }
                _ => {
                    let t_rows = ((h1 - 8.0) / 8.0) as usize;
                    for row in 0..t_rows.min(6) {
                        for col in 0..5usize {
                            s.push_str(&format!(
                                "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"7\" height=\"5\" fill=\"#b8c8d8\" stroke=\"#5a7898\" stroke-width=\"0.3\" rx=\"0.5\"/>",
                                x0 + 3.0 + col as f64 * 9.0, y0 + 5.0 + row as f64 * 8.0
                            ));
                        }
                    }
                    for row in 0..4usize {
                        for col in 0..2usize {
                            s.push_str(&format!(
                                "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"10\" height=\"7\" fill=\"#d0c8e0\" stroke=\"#7060a0\" stroke-width=\"0.4\" rx=\"0.3\"/>",
                                x0 + 52.0 + col as f64 * 12.0, y0 + 5.0 + row as f64 * 12.0
                            ));
                        }
                    }
                    round_table!(s, x0 + 98.0, y0 + h1 * 0.28, 9.0, 4);
                    round_table!(s, x0 + 98.0, y0 + h1 * 0.72, 9.0, 4);
                    round_table!(s, x0 + 128.0, y0 + h1 * 0.5, 9.0, 4);
                }
            }
            if h2 >= 12.0 {
                desk!(s, x0 + 4.0, y1 + 3.0);
                if size_tier >= 1 {
                    desk!(s, x0 + 24.0, y1 + 3.0);
                }
                let sw = (plan_w * 0.32).min(48.0);
                s.push_str(&format!(
                    "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"5\" fill=\"#c8d8b8\" stroke=\"#5a7050\" stroke-width=\"0.4\" rx=\"0.3\"/>",
                    xr - sw - 4.0, y1 + 4.0, sw
                ));
            }
        }

        // ── Civic ──────────────────────────────────────────────────────────
        "civic" => {
            let office_n: usize = match size_tier {
                0 => 2,
                1 => 4,
                _ => 5,
            };
            let conf_n: usize = match size_tier {
                0 => 1,
                _ => 2,
            };
            let ocols = if office_n > 3 { 2usize } else { 1 };
            let oper_col = office_n.div_ceil(ocols);
            let oh = ((h1 - 2.0) / oper_col as f64).min(12.0);
            for i in 0..oper_col {
                let oy = y0 + 1.0 + oh * i as f64;
                if oy + oh > y0 + h1 - 1.0 {
                    break;
                }
                s.push_str(&format!(
                    "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"13\" height=\"{:.1}\" fill=\"#e8e0d0\" stroke=\"{}\" stroke-width=\"0.4\"/>",
                    x0 + 1.0, oy, oh - 0.5, accent
                ));
            }
            if ocols == 2 {
                for i in 0..(office_n - oper_col) {
                    let oy = y0 + 1.0 + oh * i as f64;
                    if oy + oh > y0 + h1 - 1.0 {
                        break;
                    }
                    s.push_str(&format!(
                        "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"13\" height=\"{:.1}\" fill=\"#e8e0d0\" stroke=\"{}\" stroke-width=\"0.4\"/>",
                        x0 + 16.0, oy, oh - 0.5, accent
                    ));
                }
            }
            let court_w = if size_tier == 2 { 36.0f64 } else { 0.0 };
            let conf_zone_x = xr - (conf_n as f64 * 32.0 + court_w + 2.0);
            for ci in 0..conf_n {
                let cx = conf_zone_x + ci as f64 * 32.0;
                if cx < x0 + 34.0 {
                    continue;
                }
                s.push_str(&format!(
                    "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"28\" height=\"{:.1}\" fill=\"#e8e0d0\" stroke=\"{}\" stroke-width=\"0.5\"/>",
                    cx, y0 + 1.0, h1 - 2.0, accent
                ));
                let cth = ((h1 - 2.0) * 0.48).min(12.0);
                let cty = y0 + 1.0 + ((h1 - 2.0) - cth) / 2.0;
                s.push_str(&format!(
                    "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"18\" height=\"{:.1}\" fill=\"#d4c8a0\" stroke=\"#8b7a40\" stroke-width=\"0.4\" rx=\"0.5\"/>",
                    cx + 4.0, cty, cth
                ));
            }
            if size_tier == 2 {
                let crx = xr - 34.0;
                s.push_str(&format!(
                    "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"30\" height=\"{:.1}\" fill=\"#f0e8d8\" stroke=\"{}\" stroke-width=\"0.6\"/>",
                    crx, y0 + 1.0, h1 - 2.0, accent
                ));
                let cr_rows = ((h1 - 8.0) / 7.0) as usize;
                for row in 0..cr_rows.min(4) {
                    for col in 0..3usize {
                        let sy = y0 + 3.0 + row as f64 * 7.0;
                        if sy + 4.0 > y0 + h1 - 3.0 {
                            break;
                        }
                        s.push_str(&format!(
                            "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"7\" height=\"4\" fill=\"#b8c8d8\" stroke=\"#5a7898\" stroke-width=\"0.3\" rx=\"0.3\"/>",
                            crx + 2.0 + col as f64 * 9.0, sy
                        ));
                    }
                }
            }
            let rec_start = x0 + 2.0 + 14.0 * ocols as f64 + 3.0;
            let rec_end = conf_zone_x - 2.0;
            if rec_end - rec_start >= 8.0 {
                s.push_str(&format!(
                    "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"4\" fill=\"#c8b48a\" stroke=\"#8b6a40\" stroke-width=\"0.4\" rx=\"0.3\"/>",
                    rec_start, y0 + 2.0, (rec_end - rec_start).min(28.0)
                ));
            }
            if h2 >= 12.0 {
                let sr_w = (plan_w * 0.38).min(58.0);
                s.push_str(&format!(
                    "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{:.1}\" fill=\"#e0e8e0\" stroke=\"{}\" stroke-width=\"0.5\"/>",
                    x0 + 1.0, y1, sr_w, h2 * 0.82, accent
                ));
                round_table!(s, x0 + 1.0 + sr_w / 2.0, y1 + h2 * 0.40, 8.0, 4);
                s.push_str(&format!(
                    "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"16\" height=\"{:.1}\" fill=\"#e8f0f8\" stroke=\"#7090a8\" stroke-width=\"0.5\" rx=\"0.3\"/>",
                    xr - 20.0, y1 + 1.0, (h2 * 0.65).min(h2 - 2.0)
                ));
            }
            if h3 >= 8.0 {
                door!(s, x0 + 4.0, y2, (h3 * 0.80).min(14.0));
                s.push_str(&format!(
                    "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"32\" height=\"4\" fill=\"#c8b48a\" stroke=\"#8b6a40\" stroke-width=\"0.3\" rx=\"0.3\"/>",
                    x0 + 22.0, y2 + 2.0
                ));
            }
        }

        _ => {}
    }

    s.push_str(&format!(
        "<text x=\"108\" y=\"110\" font-size=\"5.5\" fill=\"{}\" font-family=\"sans-serif\" text-anchor=\"middle\" letter-spacing=\"1.2\">CORE</text>",
        accent
    ));

    s.push_str("</svg>");
    s
}
