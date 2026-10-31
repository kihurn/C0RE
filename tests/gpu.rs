use c0re::gpu::{GpuStats, parse_nvidia_smi};

#[test]
fn parses_one_gpu() {
    let out = "NVIDIA GeForce RTX 4070 SUPER, 37, 7123, 12282, 54, 120.45\n";
    assert_eq!(
        parse_nvidia_smi(out),
        Some(GpuStats {
            name: "NVIDIA GeForce RTX 4070 SUPER".into(),
            util_percent: Some(37),
            mem_used_mb: Some(7123),
            mem_total_mb: Some(12282),
            temp_c: Some(54),
            power_w: Some(120),
        })
    );
}

#[test]
fn unsupported_fields_are_missing_not_zero() {
    let out = "NVIDIA GeForce RTX 4070 SUPER, 3, 900, 12282, [N/A], [Not Supported]\n";
    let g = parse_nvidia_smi(out).unwrap();
    assert_eq!((g.util_percent, g.temp_c, g.power_w), (Some(3), None, None));
}

#[test]
fn first_gpu_only_and_garbage_rejected() {
    let out = "GPU A, 10, 1, 2, 3, 4\nGPU B, 90, 1, 2, 3, 4\n";
    assert_eq!(parse_nvidia_smi(out).unwrap().name, "GPU A");
    assert_eq!(parse_nvidia_smi(""), None);
    assert_eq!(parse_nvidia_smi("NVIDIA-SMI has failed because it couldn't communicate"), None);
}
