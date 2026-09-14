//! Joinery under a sloping roof: a slatted panel takes the roof line as its
//! top, slat by slat, with its backing cut along it.

use newera_core::{Command, Document, FurnitureId, Point2};
use serde_json::{Value, json};

use crate::{Build, PARAMS_KEY, assemble, carry_embedded, generate, merged};

/// Fits the joinery group `id` to the roof above it. Replies `{id, top}`.
///
/// # Errors
/// Groups that aren't slatted panels, and panels with no roof above.
pub fn fit_joinery_to_roof(
    doc: &mut Document,
    id: FurnitureId,
    above: f64,
) -> Result<Value, String> {
    let home = doc.home();
    let group = home
        .furniture
        .iter()
        .find(|f| f.id == id)
        .cloned()
        .ok_or_else(|| format!("{id} not found"))?;
    let stored = group
        .properties
        .get(PARAMS_KEY)
        .ok_or_else(|| format!("{id} is not joinery"))?;
    let Build::Slats(mut panel) = merged(stored, &json!({}))? else {
        return Err(format!(
            "{id}: só painéis ripados acompanham o telhado por enquanto; paredes, vidros e painéis simples usam fit_roof direto."
        ));
    };
    let view = home.level_view(home.current_level());
    // Along the panel's width at its back, every 5 cm.
    let back = -group.depth / 2.0 + 0.5;
    let steps = ((panel.w / 5.0).ceil() as usize).max(1);
    let mut samples = Vec::new();
    let mut any = false;
    for i in 0..=steps {
        #[allow(clippy::cast_precision_loss)]
        let x = panel.w * i as f64 / steps as f64;
        let at: Point2 = group.to_plan((x - panel.w / 2.0, back));
        let h = newera_core::roof_height_at(&view, at, above).map(|h| h - group.elevation);
        any |= h.is_some();
        samples.push([x, h.unwrap_or(panel.h)]);
    }
    if !any {
        return Err(format!("{id}: não há telhado inclinado acima do painel."));
    }
    // Keep the bends only.
    let mut top: Vec<[f64; 2]> = Vec::new();
    for (i, s) in samples.iter().enumerate() {
        let keep = i == 0
            || i + 1 == samples.len()
            || ((samples[i + 1][1] - s[1]) - (s[1] - samples[i - 1][1])).abs() > 0.2;
        if keep {
            top.push([(s[0] * 10.0).round() / 10.0, (s[1] * 10.0).round() / 10.0]);
        }
    }
    panel.h = top.iter().map(|t| t[1]).fold(0.0, f64::max);
    panel.top.clone_from(&top);
    let build = Build::Slats(panel);
    let output = generate(&build)?;
    let mut next = || doc.new_furniture_id();
    let mut rebuilt = assemble(
        &build,
        &output,
        group.id,
        group.position,
        group.angle,
        group.elevation,
        &mut next,
    );
    rebuilt.name.clone_from(&group.name);
    rebuilt.level = group.level;
    carry_embedded(&group, &mut rebuilt);
    doc.execute(Command::update(rebuilt))
        .map_err(|e| e.to_string())?;
    Ok(json!({"id": id.to_string(), "top": top}))
}

#[cfg(test)]
mod tests {
    use newera_core::{Furniture, Home};

    use super::*;
    use crate::SlatsParams;

    #[test]
    fn a_slatted_panel_under_a_shed_roof_follows_the_slope() {
        let mut home = Home::default();
        // A roof sloping along x: 100 cm of height over 200 cm.
        let run: f64 = 200.0;
        let rise: f64 = 100.0;
        home.furniture.push(Furniture {
            id: FurnitureId(1),
            catalog: "box".into(),
            name: "Água".into(),
            position: Point2::new(100.0, 50.0),
            elevation: 199.0,
            width: run.hypot(rise),
            depth: 300.0,
            height: 2.0,
            roll: -rise.atan2(run).to_degrees(),
            ..Furniture::default()
        });
        let mut doc = Document::new(home);
        let build = Build::Slats(SlatsParams {
            w: 160.0,
            h: 240.0,
            ..SlatsParams::default()
        });
        let output = generate(&build).unwrap();
        let id = doc.new_furniture_id();
        let group = {
            let mut next = || doc.new_furniture_id();
            assemble(
                &build,
                &output,
                id,
                Point2::new(100.0, 50.0),
                0.0,
                0.0,
                &mut next,
            )
        };
        doc.execute(Command::insert(group)).unwrap();
        let reply = fit_joinery_to_roof(&mut doc, id, 5.0).unwrap();
        let top = reply["top"].as_array().unwrap();
        let (first, last) = (
            top[0][1].as_f64().unwrap(),
            top[top.len() - 1][1].as_f64().unwrap(),
        );
        assert!(
            ((last - first).abs() - 80.0).abs() < 4.0,
            "80 cm over 160 cm: {reply}"
        );
        // The panel is as tall as the roof's high side.
        let group = doc.home().furniture.iter().find(|f| f.id == id).unwrap();
        assert!(
            (group.height - first.max(last)).abs() < 3.0,
            "{}",
            group.height
        );
    }
}
