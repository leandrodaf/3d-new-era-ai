use std::time::Instant;
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = std::path::Path::new(&args[1]);
    let cache = std::env::temp_dir().join("newera-probe-cache");
    let (project, _) = newera_core::open_project(path, &cache).expect("open");
    let mut doc = newera_core::Document::default();
    project.load_into(&mut doc);
    let shared = newera_core::SharedDocument::new(doc);
    for (name, a) in args[2..].chunks(2).map(|c| (c[0].clone(), c[1].clone())) {
        let t = Instant::now();
        let r = newera_mcp::call(shared.clone(), &name, serde_json::from_str(&a).unwrap());
        let text = match &r {
            Ok(r) => serde_json::to_string(&r.content).unwrap(),
            Err(e) => e.clone(),
        };
        eprintln!("{name} {a}: {:?} {} bytes", t.elapsed(), text.len());
        if std::env::var("SHOW").is_ok() {
            println!("{text}");
        }
    }
}
