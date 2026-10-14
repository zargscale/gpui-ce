use super::*;

#[test]
fn requirements_are_resolved_again_for_each_candidate_and_clone() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let calls = seen.clone();
    let policy = WgpuDeviceRequirements::for_adapter(move |features, limits| {
        calls
            .lock()
            .unwrap()
            .push((features, limits.max_storage_textures_per_shader_stage));
        WgpuDeviceRequirements {
            features: features & wgpu::Features::FLOAT32_FILTERABLE,
            limits: Some(limits.clone()),
            ..Default::default()
        }
    });
    let big = wgpu::Limits {
        max_storage_textures_per_shader_stage: 16,
        ..Default::default()
    };
    let small = wgpu::Limits {
        max_storage_textures_per_shader_stage: 4,
        ..Default::default()
    };
    let a = policy.resolve_for_adapter(wgpu::Features::FLOAT32_FILTERABLE, &big);
    let b = policy
        .clone()
        .resolve_for_adapter(wgpu::Features::empty(), &small);
    assert_eq!(a.features, wgpu::Features::FLOAT32_FILTERABLE);
    assert_eq!(a.limits.unwrap().max_storage_textures_per_shader_stage, 16);
    assert_eq!(b.features, wgpu::Features::empty());
    assert_eq!(b.limits.unwrap().max_storage_textures_per_shader_stage, 4);
    assert!(a.adapter_requirements.is_none() && b.adapter_requirements.is_none());
    assert_eq!(
        seen.lock().unwrap().len(),
        2,
        "fallback/recovery must not reuse resolved capabilities"
    );
}

#[test]
fn static_requirements_and_strictest_merge_are_preserved() {
    let mut policy = WgpuDeviceRequirements::for_adapter(|_, _| WgpuDeviceRequirements {
        features: wgpu::Features::FLOAT32_FILTERABLE,
        limits: Some(wgpu::Limits {
            max_buffer_size: 512 << 20,
            min_uniform_buffer_offset_alignment: 64,
            ..Default::default()
        }),
        ..Default::default()
    });
    policy.features = wgpu::Features::DEPTH_CLIP_CONTROL;
    policy.limits = Some(wgpu::Limits {
        max_buffer_size: 256 << 20,
        min_uniform_buffer_offset_alignment: 16,
        ..Default::default()
    });
    let fixed = WgpuDeviceRequirements {
        features: policy.features,
        limits: policy.limits.clone(),
        ..Default::default()
    };
    let unchanged = fixed.resolve_for_adapter(wgpu::Features::empty(), &wgpu::Limits::default());
    assert_eq!(
        unchanged.features, fixed.features,
        "explicit static requests are not silently clamped"
    );
    assert_eq!(unchanged.limits, fixed.limits);
    let merged = policy.resolve_for_adapter(wgpu::Features::empty(), &wgpu::Limits::default());
    assert_eq!(
        merged.features,
        wgpu::Features::DEPTH_CLIP_CONTROL | wgpu::Features::FLOAT32_FILTERABLE
    );
    let limits = merged.limits.unwrap();
    assert_eq!(limits.max_buffer_size, 512 << 20);
    assert_eq!(limits.min_uniform_buffer_offset_alignment, 16);
    let empty = WgpuDeviceRequirements::default()
        .resolve_for_adapter(wgpu::Features::empty(), &wgpu::Limits::default());
    assert!(empty.features.is_empty() && empty.limits.is_none());
}

#[test]
fn headless_creation_uses_actual_adapter_and_retains_policy_for_recreation() {
    let observed = Arc::new(Mutex::new(Vec::new()));
    let calls = observed.clone();
    let policy = WgpuDeviceRequirements::for_adapter(move |features, limits| {
        calls.lock().unwrap().push((features, limits.clone()));
        WgpuDeviceRequirements {
            features: features & wgpu::Features::DEPTH_CLIP_CONTROL,
            limits: Some(limits.clone()),
            ..Default::default()
        }
    });
    for retained in [&policy, &policy.clone()] {
        let before = observed.lock().unwrap().len();
        let context = WgpuContext::new_headless(Some(retained)).expect("offscreen GPU required");
        let seen = observed.lock().unwrap();
        assert!(
            seen.len() > before,
            "create_device did not resolve candidate requirements"
        );
        let (features, limits) = seen.last().unwrap();
        assert_eq!(*features, context.adapter.features());
        assert_eq!(*limits, context.adapter.limits());
        assert!(
            context
                .device
                .features()
                .contains(*features & wgpu::Features::DEPTH_CLIP_CONTROL)
        );
        assert_eq!(
            context
                .device
                .limits()
                .max_storage_textures_per_shader_stage,
            limits.max_storage_textures_per_shader_stage
        );
    }
}
