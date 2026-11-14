use super::*;

#[test]
fn tips_and_adv_share_the_script_text_origin() {
    let (assets, _) = super::tests::empty_core_assets(false);
    let mut vm = ScriptRuntime::boot(12, ScriptRuntimeConfig::default());
    vm.font_state.set_profile(crate::font::PalFontProfile::Reference);
    vm.text_state.initialized = true;
    vm.text_state.init_args = [0, 532, 360, 110, 760, 584, 280, 26];
    vm.rebuild_adv_text_render_cache(&assets, Nls::ShiftJis, None, "<d1>Aligned</d>");
    let cache = vm.text_state.render_cache.as_ref().unwrap();
    assert_eq!(cache.sprite_y + cache.text_origin_y as i32, vm.text_state.init_body_y(),
        "script-drawn TIPS overlays must share the ADV baseline");
}

#[test]
fn markup_lines_get_fresh_outline_glyphs_at_device_density() {
    let mut font = PalFontSystem::new();
    font.set_profile(crate::font::PalFontProfile::Reference);
    font.set_bitmap_text_enabled(false);
    font.set_font_size(26);
    font.set_color(0xffff_ffff, 0xff00_0000);
    let text = "Dense<br>Marked";
    let (w, h, low) = font.rasterize(text);
    let (dw, dh, dense) = font.rasterize_dense(text, 2);
    assert_eq!((dw, dh), (w * 2, h * 2));
    assert_eq!(dense.len(), (dw * dh * 4) as usize);
    assert_ne!(dense, crate::scene::upscale_rgba(&low, w, h, 2),
        "native markup must not magnify a low-resolution whole-line bitmap");
}

#[test]
fn script_text_surfaces_keep_dense_pixels_when_appended() {
    let (assets, points) = super::tests::empty_core_assets(false);
    let mut vm = ScriptRuntime::boot(12, ScriptRuntimeConfig::default());
    vm.font_state.set_profile(crate::font::PalFontProfile::Reference);
    vm.font_state.set_bitmap_text_enabled(false);
    vm.set_text_density(2);
    let mut sprites = SpriteSystem::new();
    let canvas = sprites.create_rgba_sprite(180, 64, vec![0; 180 * 64 * 4],
        PalVec3::new(5, 6, 0), 0, "tips-canvas").unwrap();
    vm.game_sprites.insert(3, canvas);
    let text = vm.store_dynamic_string("Aligned".to_owned());
    for x in [0, 80] {
        vm.stack = vec![0, 2, x, text, 3];
        assert_eq!(vm.dispatch_extcall(3, 22, &[], &assets, &points, None,
            Some(&mut sprites), None, None, None), ExtCallOutcome::Value(1));
        let surface = sprites.surface(sprites.get(canvas).unwrap().surface).unwrap();
        let dense = surface.to_scene_texture().hires.expect("all script text needs dense glyphs");
        assert_eq!(dense.density, 2);
        assert!(dense.pixels.chunks_exact(4).any(|p| p[3] != 0));
    }
    assert_eq!(sprites.get(canvas).unwrap().position, PalVec3::new(5, 6, 0));
}

#[test]
fn yureaka_host_only_d3d_queries_preserve_stack_and_report_unavailable() {
    let (mut assets, points) = super::tests::empty_core_assets(false);
    assets.native_build = crate::assets::NativeBuild::Yureaka;
    let mut runtime = ScriptRuntime::boot(12, ScriptRuntimeConfig::default());
    for index in 93..=95 {
        runtime.stack = vec![77];
        assert_eq!(runtime.dispatch_extcall(18, index, &[], &assets, &points, None, None, None, None, None), ExtCallOutcome::Value(0));
        assert_eq!(runtime.stack, vec![77]);
    }
    runtime.stack = vec![77, 0, 1];
    assert_eq!(runtime.dispatch_extcall(18, 100, &[], &assets, &points, None, None, None, None, None), ExtCallOutcome::Value(0));
    assert_eq!(runtime.stack, vec![77]);
}

#[test]
fn yureaka_thick_outline_produces_visible_text_after_dispatch() {
    let (mut assets, points) = super::tests::empty_core_assets(false);
    assets.native_build = crate::assets::NativeBuild::Yureaka;
    let mut runtime = ScriptRuntime::boot(12, ScriptRuntimeConfig::default());
    runtime.font_state.set_font_size(26);
    runtime.font_state.set_color(0xFFFF_FFFF, 0xFF00_0000);
    runtime.font_state.set_effect(3);
    runtime.stack.push(0);
    runtime.dispatch_extcall(9, 55, &[], &assets, &points, None, None, None, None, None);
    let (_, _, pixels) = runtime.font_state.rasterize("Visible");
    assert!(pixels.chunks_exact(4).any(|pixel| pixel[3] != 0), "Yureaka effect 3 must not erase text");
}

#[test]
fn explicit_font_overrides_single_language_bitmap_and_invalid_font_errors() {
    let root = std::env::temp_dir().join(format!("sena_font_selection_{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("selected.ttf");
    std::fs::write(&path, include_bytes!("../default.ttf")).unwrap();
    let mut resources = ResourceManager::new(&root, Nls::ShiftJis);
    let mut runtime = ScriptRuntime::boot(12, ScriptRuntimeConfig::default());
    runtime.font_state.set_type(4);
    runtime.load_configured_font(&mut resources, Nls::ShiftJis, Some(&root), Some(&path), TextLang::Jp, false).unwrap();
    assert!(runtime.font_state.draws_outline_text());
    assert_eq!(runtime.loaded_face_path.as_deref(), Some(path.as_path()));
    std::fs::write(&path, b"not a font").unwrap();
    assert!(runtime.load_configured_font(&mut resources, Nls::ShiftJis, Some(&root), Some(&path), TextLang::Jp, false).is_err());
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(root).unwrap();
}
