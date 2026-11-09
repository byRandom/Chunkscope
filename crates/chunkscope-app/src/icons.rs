//! Line icons drawn in code, on a 16-unit grid scaled to the given rect.

use eframe::egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, pos2, vec2};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Map,
    Layers,
    Search,
    Info,
    Eye,
    EyeOff,
    Hand,
    Plus,
    Minus,
    Target,
    Grid,
    Slime,
    Dice,
    Pin,
}

/// Draw `icon` inside `rect` with a 1.4 px stroke of `color`.
pub fn paint(painter: &Painter, rect: Rect, icon: Icon, color: Color32) {
    let s = rect.width().min(rect.height()) / 16.0;
    let o = rect.center() - vec2(8.0 * s, 8.0 * s);
    let p = |x: f32, y: f32| -> Pos2 { o + vec2(x * s, y * s) };
    let stroke = Stroke::new(1.4, color);
    let line = |pts: &[(f32, f32)]| {
        painter.add(Shape::line(
            pts.iter().map(|&(x, y)| p(x, y)).collect(),
            stroke,
        ));
    };
    let closed = |pts: &[(f32, f32)]| {
        painter.add(Shape::closed_line(
            pts.iter().map(|&(x, y)| p(x, y)).collect(),
            stroke,
        ));
    };
    let rect_at = |x0: f32, y0: f32, x1: f32, y1: f32, r: f32| {
        painter.rect_stroke(
            Rect::from_min_max(p(x0, y0), p(x1, y1)),
            r * s,
            stroke,
            eframe::egui::StrokeKind::Middle,
        );
    };
    match icon {
        Icon::Map => {
            closed(&[
                (2.0, 4.0),
                (6.0, 2.5),
                (10.0, 4.0),
                (14.0, 2.5),
                (14.0, 12.0),
                (10.0, 13.5),
                (6.0, 12.0),
                (2.0, 13.5),
            ]);
            line(&[(6.0, 2.5), (6.0, 12.0)]);
            line(&[(10.0, 4.0), (10.0, 13.5)]);
        }
        Icon::Layers => {
            closed(&[(8.0, 2.5), (14.0, 5.5), (8.0, 8.5), (2.0, 5.5)]);
            line(&[(2.0, 8.5), (8.0, 11.5), (14.0, 8.5)]);
            line(&[(2.0, 11.0), (8.0, 14.0), (14.0, 11.0)]);
        }
        Icon::Search => {
            painter.circle_stroke(p(7.0, 7.0), 4.5 * s, stroke);
            line(&[(10.3, 10.3), (14.0, 14.0)]);
        }
        Icon::Info => {
            painter.circle_stroke(p(8.0, 8.0), 6.0 * s, stroke);
            line(&[(8.0, 7.2), (8.0, 11.5)]);
            painter.circle_filled(p(8.0, 4.9), 0.9 * s, color);
        }
        Icon::Eye | Icon::EyeOff => {
            let mut upper = Vec::new();
            let mut lower = Vec::new();
            for i in 0..=12 {
                let t = i as f32 / 12.0;
                let x = 1.5 + 13.0 * t;
                let h = 4.2 * (std::f32::consts::PI * t).sin();
                upper.push(p(x, 8.0 - h));
                lower.push(p(x, 8.0 + h));
            }
            lower.reverse();
            upper.extend(lower);
            painter.add(Shape::closed_line(upper, stroke));
            painter.circle_stroke(p(8.0, 8.0), 2.0 * s, stroke);
            if icon == Icon::EyeOff {
                line(&[(2.5, 13.5), (13.5, 2.5)]);
            }
        }
        Icon::Hand => {
            line(&[(5.0, 9.0), (5.0, 4.0)]);
            line(&[(7.3, 8.0), (7.3, 2.8)]);
            line(&[(9.6, 8.0), (9.6, 3.4)]);
            line(&[(11.9, 8.5), (11.9, 5.0)]);
            line(&[
                (11.9, 8.5),
                (11.9, 10.5),
                (10.0, 14.0),
                (6.0, 14.0),
                (3.0, 9.5),
                (3.5, 8.5),
                (5.0, 9.0),
            ]);
        }
        Icon::Plus => {
            line(&[(8.0, 3.0), (8.0, 13.0)]);
            line(&[(3.0, 8.0), (13.0, 8.0)]);
        }
        Icon::Minus => line(&[(3.0, 8.0), (13.0, 8.0)]),
        Icon::Target => {
            painter.circle_stroke(p(8.0, 8.0), 4.5 * s, stroke);
            line(&[(8.0, 1.5), (8.0, 4.5)]);
            line(&[(8.0, 11.5), (8.0, 14.5)]);
            line(&[(1.5, 8.0), (4.5, 8.0)]);
            line(&[(11.5, 8.0), (14.5, 8.0)]);
        }
        Icon::Grid => {
            rect_at(2.5, 2.5, 13.5, 13.5, 1.5);
            line(&[(6.2, 2.5), (6.2, 13.5)]);
            line(&[(9.8, 2.5), (9.8, 13.5)]);
            line(&[(2.5, 6.2), (13.5, 6.2)]);
            line(&[(2.5, 9.8), (13.5, 9.8)]);
        }
        Icon::Slime => {
            rect_at(2.5, 3.5, 13.5, 13.5, 3.0);
            painter.circle_filled(p(6.0, 7.5), 1.0 * s, color);
            painter.circle_filled(p(10.0, 7.5), 1.0 * s, color);
            line(&[(6.5, 10.8), (9.5, 10.8)]);
        }
        Icon::Dice => {
            rect_at(2.5, 2.5, 13.5, 13.5, 2.5);
            for (x, y) in [
                (5.5, 5.5),
                (10.5, 10.5),
                (8.0, 8.0),
                (10.5, 5.5),
                (5.5, 10.5),
            ] {
                painter.circle_filled(p(x, y), 0.9 * s, color);
            }
        }
        Icon::Pin => {
            let mut pts = Vec::new();
            for i in 0..=16 {
                let a = std::f32::consts::PI * (0.8 + 1.4 * i as f32 / 16.0);
                pts.push(pos2(8.0 + 4.5 * a.cos(), 6.5 + 4.5 * a.sin()));
            }
            pts.push(pos2(8.0, 14.5));
            painter.add(Shape::closed_line(
                pts.into_iter().map(|q| p(q.x, q.y)).collect(),
                stroke,
            ));
            painter.circle_stroke(p(8.0, 6.5), 1.6 * s, stroke);
        }
    }
}
