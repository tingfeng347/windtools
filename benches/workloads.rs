use ratatui::{Terminal, backend::TestBackend, buffer::Buffer, layout::Rect, widgets::Widget};
use std::{hint::black_box, time::Instant};
use windtools::{
    app::{App, Options, Screen},
    meadow::{Meadow, MeadowView},
    model::Task,
    store::Store,
    ui,
};

fn main() {
    let store = Store::in_memory().unwrap();
    for index in 0..10_000 {
        store
            .insert(&Task::new(&format!("事项 {index} 中文测试"), "", None, 100).unwrap())
            .unwrap();
    }
    let start = Instant::now();
    let tasks = store.list().unwrap();
    println!(
        "load_10000_ms={:.3}",
        start.elapsed().as_secs_f64() * 1000.0
    );
    let start = Instant::now();
    for _ in 0..100 {
        black_box(
            tasks
                .iter()
                .filter(|task| task.title.contains("999"))
                .count(),
        );
    }
    println!(
        "search_10000_mean_ms={:.3}",
        start.elapsed().as_secs_f64() * 10.0
    );
    let mut app = App::new(
        store,
        Screen::Todo,
        Options {
            demo: true,
            ..Options::default()
        },
    )
    .unwrap();
    app.search.text = "999".into();
    let start = Instant::now();
    for _ in 0..100 {
        app.refilter();
        black_box(&app.filtered);
    }
    println!(
        "ui_filter_10000_mean_ms={:.3}",
        start.elapsed().as_secs_f64() * 10.0
    );
    for (width, height) in [(120, 40), (240, 80)] {
        let mut scene = Meadow::new(42);
        scene.prepare(width, height);
        let area = Rect::new(0, 0, width, height);
        let mut buffer = Buffer::empty(area);
        let mut timings = Vec::new();
        for _ in 0..300 {
            let start = Instant::now();
            scene.tick(1.0 / 30.0);
            MeadowView {
                meadow: &scene,
                ascii: false,
                mono: false,
                ansi: false,
            }
            .render(area, &mut buffer);
            black_box(&buffer);
            timings.push(start.elapsed().as_secs_f64() * 1000.0);
        }
        timings.sort_by(f64::total_cmp);
        println!(
            "meadow_{width}x{height}_p50_ms={:.3} p95_ms={:.3} max_ms={:.3}",
            timings[150], timings[285], timings[299]
        );
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        app.search.text.clear();
        app.refilter();
        let start = Instant::now();
        for _ in 0..100 {
            terminal.draw(|frame| ui::render(frame, &mut app)).unwrap();
        }
        println!(
            "todo_{width}x{height}_10000_mean_frame_ms={:.3}",
            start.elapsed().as_secs_f64() * 10.0
        );
    }
}
