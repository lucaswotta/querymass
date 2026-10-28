use eframe::egui;
use std::io::Write;
pub fn capture(ctx: &egui::Context) {
    let Some(directory) = std::env::var_os("QUERYMASS_QA_CAPTURE").map(std::path::PathBuf::from)
    else {
        return;
    };
    let request = directory.join("capture.request");
    if let Ok(name) = std::fs::read_to_string(&request) {
        if let Err(error) = std::fs::remove_file(&request) {
            eprintln!("QA capture request: {error}");
            return;
        }
        let name: String = name
            .chars()
            .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '-')
            .collect();
        ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::new(name)));
    }
    for event in ctx.input(|input| input.events.clone()) {
        if let egui::Event::Screenshot {
            user_data, image, ..
        } = event
        {
            let Some(name) = user_data
                .data
                .as_ref()
                .and_then(|data| data.downcast_ref::<String>())
            else {
                continue;
            };
            let write = || -> std::io::Result<()> {
                let path = directory.join(format!("{name}.ppm"));
                let temporary = path.with_extension("ppm.tmp");
                let mut file = std::io::BufWriter::new(std::fs::File::create(&temporary)?);
                write!(file, "P6\n{} {}\n255\n", image.size[0], image.size[1])?;
                let pixels: Vec<u8> = image
                    .pixels
                    .iter()
                    .flat_map(|pixel| pixel.to_array().into_iter().take(3))
                    .collect();
                file.write_all(&pixels)?;
                file.flush()?;
                std::fs::rename(temporary, path)?;
                Ok(())
            };
            if let Err(error) = write() {
                eprintln!("QA capture: {error}");
            }
        }
    }
    ctx.request_repaint_after(std::time::Duration::from_millis(100));
}

pub fn input(
    ctx: &egui::Context,
    raw: &mut egui::RawInput,
    queue: &mut std::collections::VecDeque<Vec<egui::Event>>,
) {
    let Some(directory) = std::env::var_os("QUERYMASS_QA_CAPTURE").map(std::path::PathBuf::from)
    else {
        return;
    };
    raw.focused = true;
    for viewport in raw.viewports.values_mut() {
        viewport.focused = Some(true);
    }
    let request = directory.join("input.request");
    if queue.is_empty()
        && let Ok(bytes) = std::fs::read(&request)
    {
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(
            egui::WindowLevel::AlwaysOnTop,
        ));
        let _ = std::fs::remove_file(directory.join("input.done"));
        if let Err(error) = std::fs::remove_file(&request) {
            eprintln!("QA input request: {error}");
            return;
        }
        if let Ok(actions) = serde_json::from_slice::<Vec<serde_json::Value>>(&bytes) {
            for action in actions {
                let point = |key: &str| {
                    action[key].as_array().map(|coordinates| {
                        egui::pos2(
                            coordinates[0].as_f64().unwrap_or_default() as f32,
                            coordinates[1].as_f64().unwrap_or_default() as f32,
                        )
                    })
                };
                if let Some(position) = point("click") {
                    let button = if action["right"].as_bool().unwrap_or(false) {
                        egui::PointerButton::Secondary
                    } else {
                        egui::PointerButton::Primary
                    };
                    queue.push_back(vec![egui::Event::PointerMoved(position)]);
                    queue.push_back(vec![
                        egui::Event::PointerButton {
                            pos: position,
                            button,
                            pressed: true,
                            modifiers: egui::Modifiers::NONE,
                        },
                        egui::Event::PointerButton {
                            pos: position,
                            button,
                            pressed: false,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ]);
                } else if let Some(text) = action["text"].as_str() {
                    queue.push_back(vec![egui::Event::Paste(text.into())]);
                } else if let Some(name) = action["key"].as_str() {
                    let key = match name {
                        "Enter" => egui::Key::Enter,
                        "Escape" => egui::Key::Escape,
                        "A" => egui::Key::A,
                        "Tab" => egui::Key::Tab,
                        _ => continue,
                    };
                    let modifiers = egui::Modifiers {
                        ctrl: action["ctrl"].as_bool().unwrap_or(false),
                        shift: action["shift"].as_bool().unwrap_or(false),
                        ..Default::default()
                    };
                    queue.push_back(vec![egui::Event::Key {
                        key,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers,
                    }]);
                    queue.push_back(vec![egui::Event::Key {
                        key,
                        physical_key: None,
                        pressed: false,
                        repeat: false,
                        modifiers,
                    }]);
                } else if let (Some(from), Some(to)) = (point("from"), point("to")) {
                    queue.push_back(vec![egui::Event::PointerMoved(from)]);
                    queue.push_back(vec![egui::Event::PointerButton {
                        pos: from,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: egui::Modifiers::NONE,
                    }]);
                    for step in 1..=12 {
                        queue.push_back(vec![egui::Event::PointerMoved(
                            from.lerp(to, step as f32 / 12.0),
                        )]);
                    }
                    queue.push_back(vec![egui::Event::PointerButton {
                        pos: to,
                        button: egui::PointerButton::Primary,
                        pressed: false,
                        modifiers: egui::Modifiers::NONE,
                    }]);
                } else if let (Some(width), Some(height)) =
                    (action["width"].as_f64(), action["height"].as_f64())
                {
                    ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
                        width as f32,
                        height as f32,
                    )));
                }
                queue.push_back(Vec::new());
            }
        }
    }
    if let Some(events) = queue.pop_front() {
        raw.focused = true;
        raw.events.extend(events);
        if queue.is_empty()
            && let Err(error) = std::fs::write(directory.join("input.done"), "done")
        {
            eprintln!("QA input completion: {error}");
        }
        ctx.request_repaint();
    }
}

pub fn state(app: &super::App, ctx: &egui::Context) {
    let Some(directory) = std::env::var_os("QUERYMASS_QA_CAPTURE").map(std::path::PathBuf::from)
    else {
        return;
    };
    let profiles:Vec<_>=app.workspace.profiles.iter().map(|profile|serde_json::json!({"id":profile.id,"name":profile.connection.name,"folder":profile.connection.folder,"selected":profile.selected})).collect();
    let outputs:Vec<_>=app.outputs.iter().map(|output|serde_json::json!({"id":output.machine_id,"outcome":format!("{:?}",output.outcome),"sets":output.sets.iter().map(|set|serde_json::json!({"rows":set.rows.len(),"affected":set.affected_rows})).collect::<Vec<_>>()})).collect();
    let state = serde_json::json!({"manager":app.manager,"profiles":profiles,"folders":app.workspace.folders,"running":app.run.is_some(),"outputs":outputs,"form_error":app.form_error,"notice":app.notice,"sql_length":app.workspace.sql.len(),"pointer":ctx.input(|input|format!("{:?} down={} click={} focus={}",input.pointer.interact_pos(),input.pointer.primary_down(),input.pointer.primary_clicked(),input.focused))});
    if let Ok(bytes) = serde_json::to_vec(&state) {
        let _ = std::fs::write(directory.join("state.json"), bytes);
    }
}
