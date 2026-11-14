use super::tests::empty_core_assets;
use super::*;

struct ImageRoot(std::path::PathBuf);

impl ImageRoot {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "sena_sprite_display_{}_{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        ));
        std::fs::create_dir_all(&root).unwrap();
        for name in ["scene.png", "next.png", "face.png"] {
            let file = std::fs::File::create(root.join(name)).unwrap();
            let mut encoder = png::Encoder::new(file, 2, 2);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .write_header()
                .unwrap()
                .write_image_data(&[255; 16])
                .unwrap();
        }
        Self(root)
    }
}

impl Drop for ImageRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn reported_old_save_recovers_omitted_resource_sprites_and_new_saves_keep_large_images() {
    let root = ImageRoot::new();
    let mut resources = ResourceManager::new(&root.0, Nls::ShiftJis);
    let (assets, _) = empty_core_assets(false);
    let mut runtime = ScriptRuntime::boot(0x1000, ScriptRuntimeConfig::default());
    let mut sprites = SpriteSystem::new();
    let standing = sprites.create_rgba_sprite(
        2, 2, vec![255; 16], PalVec3::new(120, 80, 0), 0, "scene.png",
    ).unwrap();
    sprites.set_scale(standing, 0.9);
    runtime.game_sprites.insert(10, standing);
    let mut snapshot = runtime.capture_resumable_scene(Some(&sprites));
    // Earlier saves retained the source/transform binding but omitted pixels
    // for standing images above the old eight-MiB save limit.
    snapshot.sprites.clear();
    write_runtime_save_snapshot(&root.0, 0, &snapshot).unwrap();
    runtime.stack.push(0);
    assert_eq!(runtime.dispatch_save_stub(
        1, &assets, Nls::ShiftJis, Some(&mut resources), Some(&mut sprites),
    ), ExtCallOutcome::Value(1));
    let restored = runtime.game_sprites.get(&10).copied()
        .expect("old save must reload the missing standing image from its resource binding");
    let sprite = sprites.get(restored).unwrap();
    assert_eq!(sprite.position, PalVec3::new(120, 80, 0));
    assert!((sprite.scale - 0.9).abs() < 0.0001);
    let name = runtime.store_dynamic_string("next.png".into());
    runtime.stack.extend([300, 1, 10, name]);
    runtime.ext_sp_set_transition(&assets, Nls::ShiftJis, Some(&mut resources), Some(&mut sprites));
    let sprite = sprites.get(runtime.game_sprites[&10]).unwrap();
    assert!((sprite.scale - 0.9).abs() < 0.0001, "recovered scale must survive expression swaps");
    assert_eq!(sprite.position, PalVec3::new(120, 80, 0));
    let large = sprites.create_rgba_sprite(
        2048, 2048, vec![255; 2048 * 2048 * 4], PalVec3::default(), 0, "large.png",
    ).unwrap();
    runtime.game_sprites.insert(11, large);
    let snapshot = runtime.capture_resumable_scene(Some(&sprites));
    assert!(snapshot.sprites.iter().any(|record| record.slot == 11),
        "new saves must include the sixteen-MiB standing image");
    write_runtime_save_snapshot(&root.0, 1, &snapshot).unwrap();
    let restored = read_runtime_save_snapshot(&root.0, 1).unwrap();
    assert_eq!(restored.sprites.iter().find(|record| record.slot == 11).unwrap().rgba.len(), 2048 * 2048 * 4);
}

#[test]
fn reported_ruby_is_visible_above_body_text_at_logical_and_dense_sizes() {
    let (assets, _) = empty_core_assets(false);
    let mut runtime = ScriptRuntime::boot(0x1000, ScriptRuntimeConfig::default());
    runtime.font_state.set_profile(crate::font::PalFontProfile::Reference);
    runtime.font_state.set_bitmap_text_enabled(false);
    runtime.set_text_density(2);
    runtime.text_state.initialized = true;
    runtime.text_state.init_args = [0, 532, 360, 110, 760, 584, 280, 26];
    runtime.rebuild_adv_text_render_cache(&assets, Nls::ShiftJis, None, "Prefix <rreading>XY</r>.");
    let cache = runtime.text_state.render_cache.as_ref().unwrap();
    assert_eq!(cache.full_char_count, "Prefix XY.".chars().count());
    assert!(cache.ruby.iter().all(|image| image.y >= 0), "ruby must stay inside reserved text bounds, below the UI");
    for density in [1, 2] {
        let (panel, text) = match density {
            1 => (&cache.panel_rgba, &cache.text_rgba),
            _ => {
                let dense = cache.dense.as_ref().unwrap();
                (&dense.panel_rgba, &dense.text_rgba)
            }
        };
        let pixels = compose_adv_text_layer(cache, density, panel, text, None, None, None);
        let top_end = ((cache.text_origin_y + cache.lines[0].y) * density * cache.panel_width * density * 4) as usize;
        assert!(pixels[..top_end] != panel[..top_end], "ruby must render above the body at density {density}");
        let hidden = compose_adv_text_layer(cache, density, panel, text, Some((0, 0)), None, None);
        assert_eq!(hidden, *panel, "ruby must not appear before its base text is revealed");
    }
    runtime.rebuild_adv_text_render_cache(&assets, Nls::ShiftJis, None, "<rfirst>AB</r><br><rsecond>CD</r>");
    let cache = runtime.text_state.render_cache.as_ref().unwrap();
    for image in &cache.ruby {
        let line = &cache.lines[image.line];
        assert!(image.y >= 0);
        assert!(image.y as u32 + image.height < line.y);
        if image.line > 0 {
            let previous = &cache.lines[image.line - 1];
            assert!(image.y as u32 >= previous.y + previous.height,
                "ruby must not collide with the preceding body line");
        }
    }
}

#[test]
fn reported_fullscreen_narrative_uses_novel_origin_and_keeps_previous_paragraphs() {
    let (assets, _) = empty_core_assets(false);
    let mut runtime = ScriptRuntime::boot(0x1000, ScriptRuntimeConfig::default());
    runtime.font_state.set_profile(crate::font::PalFontProfile::Reference);
    runtime.text_state.initialized = true;
    runtime.text_state.init_args = [10, 536, 360, 130, 764, 577, 280, 26];
    runtime.stack.extend([610, 1280, 40, 218]);
    runtime.dispatch_text_stub(31, &assets, Nls::ShiftJis, None, None);
    runtime.stack.push(1);
    runtime.dispatch_text_stub(30, &assets, Nls::ShiftJis, None, None);
    for text in ["First paragraph.", "Second paragraph."] {
        let value = runtime.store_dynamic_string(text.into());
        runtime.stack.extend([0, value, 0x0fff_ffff, 0x0fff_ffff]);
        runtime.dispatch_text_stub(2, &assets, Nls::ShiftJis, None, None);
    }
    let body = runtime.adv_text_parts_for_render(&assets, Nls::ShiftJis).0;
    runtime.rebuild_adv_text_render_cache(&assets, Nls::ShiftJis, None, &body);
    let cache = runtime.text_state.render_cache.as_ref().unwrap();
    assert_eq!(cache.sprite_x + cache.text_origin_x as i32, 218);
    assert!(cache.sprite_y + ((cache.text_origin_y + cache.lines[0].y) as i32) < 120,
        "novel text must start near the top, not in the ADV panel");
    assert!(body.contains("First paragraph.") && body.contains("Second paragraph."),
        "novel pages must retain earlier paragraphs until text_clear");
    assert!(cache.panel_height >= 610);
    let mut sprites = SpriteSystem::new();
    let history = sprites.create_rgba_sprite(1, 1, vec![255; 4], PalVec3::default(), 0, "history:text").unwrap();
    runtime.history_state.sprite = Some(history);
    runtime.history_state.active = true;
    runtime.sync_history_sprite(&assets, Nls::ShiftJis, &mut sprites);
    assert!(!sprites.get(history).unwrap().visible,
        "preparing backlog geometry must not leave a backlog overlay over live story text");
    let toolbar = sprites.create_rgba_sprite(1, 1, vec![255; 4], PalVec3::default(), 0, "toolbar").unwrap();
    runtime.game_buttons.insert((0, 0), GameButtonEntry {
        registration_order: 0, native_flags: 0, hover_callbacks: [None; 2], handle: toolbar,
        name: "toolbar".into(), visible: true, enabled: true, locked: false, toggle: 0,
        alpha: 255, slider_offset: 0, base_position: [0; 2], slider_position: [0; 2],
        hit_rect: None, gosub_point: None, anim_resource: None, anim_play_flag: 0,
    });
    runtime.text_state.sprite = Some(history);
    sprites.view_ctrl(history, true);
    runtime.sync_adv_button_chrome_visibility(&mut sprites);
    assert!(!sprites.get(toolbar).unwrap().visible, "NVL must not show ADV toolbar/name chrome");
    runtime.text_state.visible = false;
    runtime.sync_history_sprite(&assets, Nls::ShiftJis, &mut sprites);
    assert!(sprites.get(history).unwrap().visible, "the backlog must still open when story text is hidden");
}

#[test]
fn save_load_keeps_face_registration_and_face_updates_do_not_replace_backgrounds() {
    let root = ImageRoot::new();
    let mut resources = ResourceManager::new(&root.0, Nls::ShiftJis);
    let (assets, _) = empty_core_assets(false);
    let mut runtime = ScriptRuntime::boot(0x1000, ScriptRuntimeConfig::default());
    let mut sprites = SpriteSystem::new();
    runtime.stack.extend([-1, 720, 110, 30, 0]);
    assert_eq!(runtime.ext_face_init(), ExtCallOutcome::Value(1));
    let background = sprites
        .create_rgba_sprite(2, 2, vec![255; 16], PalVec3::default(), 0, "background")
        .unwrap();
    runtime.game_sprites.insert(0, background);
    let snapshot = runtime.capture_resumable_scene(Some(&sprites));
    runtime.restore_save_snapshot(snapshot.clone());
    runtime.restore_checkpoint_scene(&snapshot, &mut sprites);
    let restored_background = runtime.game_sprites[&0];
    let name = runtime.store_dynamic_string("face.png".into());
    runtime.stack.extend([0, 0, 0, name]);
    assert_eq!(
        runtime.ext_face_set(
            &assets,
            Nls::ShiftJis,
            Some(&mut resources),
            Some(&mut sprites),
        ),
        ExtCallOutcome::Value(1)
    );
    assert_eq!(
        runtime.game_sprites[&0], restored_background,
        "face_set must not overwrite the background slot"
    );
    let face = sprites.get(runtime.game_sprites[&30]).unwrap();
    assert_eq!(face.position.x, 164.0);
    assert_eq!(face.position.y, 1078.0);
    assert!(face.smooth_upscale);
    runtime.stack.push(0);
    assert_eq!(
        runtime.ext_face_cls(Some(&mut sprites)),
        ExtCallOutcome::Value(1)
    );
    assert!(sprites.get(restored_background).is_some());
}

#[test]
fn loaded_sprite_and_expression_replacement_enable_device_filtering() {
    let root = ImageRoot::new();
    let mut resources = ResourceManager::new(&root.0, Nls::ShiftJis);
    let (mut assets, _) = empty_core_assets(false);
    assets.registered_softpal = true;
    let mut runtime = ScriptRuntime::boot(0x1000, ScriptRuntimeConfig::default());
    let mut sprites = SpriteSystem::new();
    let name = runtime.store_dynamic_string("scene".into());
    runtime.stack.extend([0, 0, 0, 10, name]);
    assert_eq!(
        runtime.ext_sp_set(
            5,
            &assets,
            Nls::ShiftJis,
            Some(&mut resources),
            Some(&mut sprites),
            None,
            false,
        ),
        ExtCallOutcome::Value(1)
    );
    assert!(
        sprites
            .get(runtime.game_sprites[&10])
            .unwrap()
            .smooth_upscale,
        "loaded artwork must enable the renderer's device filtering"
    );
    let name = runtime.store_dynamic_string("next.png".into());
    runtime.stack.extend([300, 1, 10, name]);
    assert_eq!(
        runtime.ext_sp_set_transition(
            &assets,
            Nls::ShiftJis,
            Some(&mut resources),
            Some(&mut sprites),
        ),
        ExtCallOutcome::Value(1)
    );
    assert!(
        sprites
            .get(runtime.game_sprites[&10])
            .unwrap()
            .smooth_upscale,
        "an expression replacement must keep device filtering enabled"
    );
}

#[test]
fn completed_action_scale_survives_expression_replacement_and_wrapper_commit() {
    let root = ImageRoot::new();
    let mut resources = ResourceManager::new(&root.0, Nls::ShiftJis);
    let (mut assets, _) = empty_core_assets(false);
    assets.registered_softpal = true;
    let mut runtime = ScriptRuntime::boot(0x1000, ScriptRuntimeConfig::default());
    let mut sprites = SpriteSystem::new();
    let name = runtime.store_dynamic_string("scene".into());
    runtime.stack.extend([0, 0, 0, 10, name]);
    assert_eq!(
        runtime.ext_sp_set(
            5,
            &assets,
            Nls::ShiftJis,
            Some(&mut resources),
            Some(&mut sprites),
            None,
            false,
        ),
        ExtCallOutcome::Value(1)
    );
    runtime.stack.extend([100, -10, 10, 0]);
    runtime.dispatch_action(11, Some(&mut sprites));
    runtime.stack.extend([200, 0]);
    runtime.dispatch_action(0, Some(&mut sprites));
    runtime.advance_actions(&mut sprites);
    runtime.set_pal_time(50);
    runtime.advance_actions(&mut sprites);
    assert_eq!(
        runtime.game_sprite_native_scales[&10], 100,
        "temporary scale must not be committed to the wrapper"
    );
    assert!((sprites.get(runtime.game_sprites[&10]).unwrap().info().scale - 0.95).abs() < 0.0001);
    runtime.set_pal_time(200);
    runtime.advance_actions(&mut sprites);
    let scale = sprites.get(runtime.game_sprites[&10]).unwrap().scale;
    assert!((scale - 0.9).abs() < 0.0001);

    let name = runtime.store_dynamic_string("next.png".into());
    runtime.stack.extend([300, 1, 10, name]);
    assert_eq!(
        runtime.ext_sp_set_transition(
            &assets,
            Nls::ShiftJis,
            Some(&mut resources),
            Some(&mut sprites),
        ),
        ExtCallOutcome::Value(1)
    );
    let handle = runtime.game_sprites[&10];
    assert!(
        (sprites.get(handle).unwrap().scale - 0.9).abs() < 0.0001,
        "expression replacement must retain the completed action scale"
    );
    runtime.commit_game_sprite_wrapper_visual(&mut sprites, 10, handle);
    assert!(
        (sprites.get(handle).unwrap().scale - 0.9).abs() < 0.0001,
        "a later wrapper commit must not reset the completed action scale"
    );
}
