use std::process::Command;

use super::super::super::tonemap::{Filters, Route, ToneMap, ToneSource};
use super::*;

const SOURCE: (u32, u32) = (3840, 2160);

fn pipeline(accel: HwAccel, route: Route) -> Pipeline {
    let map = ToneMap::resolve(
        ToneSource::Pq,
        Filters {
            zimg: true,
            placebo: false,
        },
    )
    .unwrap();
    Pipeline {
        accel,
        effort: Effort::Quality,
        tonemap: Some(map.routed(route)),
        frame: Some(SOURCE),
    }
}

fn listed(filter: &str) -> bool {
    let out = Command::new("ffmpeg")
        .args(["-hide_banner", "-filters"])
        .output()
        .map(|o| o.stdout)
        .unwrap_or_default();
    String::from_utf8_lossy(&out).contains(&format!(" {filter} "))
}

fn names(filter: &str) -> Vec<&str> {
    filter
        .split(',')
        .filter_map(|step| step.split('=').next())
        .collect()
}

fn refuses_to_parse(filter: &str) -> Option<String> {
    let mut cmd = Command::new("ffmpeg");
    crate::input::without_proxies(&mut cmd);
    let out = cmd
        .args(["-v", "error", "-nostdin"])
        .args(crate::input::generated("color=s=64x64:d=0.1"))
        .args(["-vf"])
        .arg(filter)
        .args(["-f", "null", "-"])
        .output()
        .ok()?;
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    [
        "Option not found",
        "No such filter",
        "Unable to parse option",
        "Error parsing",
    ]
    .iter()
    .any(|bad| stderr.contains(bad))
    .then_some(stderr)
}

#[test]
fn every_device_vector_is_spelled_in_options_this_ffmpeg_knows() {
    let mut checked = 0;
    for accel in [HwAccel::Vaapi, HwAccel::Qsv, HwAccel::Nvenc] {
        for route in [Route::Device, Route::Download] {
            let filter = pipeline(accel, route).video_filter(None).unwrap();
            let hardware: Vec<&str> = names(&filter)
                .into_iter()
                .filter(|n| n.contains("vaapi") || n.contains("qsv") || n.contains("cuda"))
                .collect();
            if hardware.iter().any(|name| !listed(name)) {
                continue;
            }
            if let Some(stderr) = refuses_to_parse(&filter) {
                panic!("{} {route:?}: {filter}\n{stderr}", accel.label());
            }
            checked += 1;
        }
    }
    eprintln!("parsed {checked} device vectors");
}

#[test]
fn a_tone_map_on_the_cpu_route_costs_cpu_and_two_slots_even_with_a_device_encoder() {
    let cpu = pipeline(HwAccel::Vaapi, Route::Cpu);
    let download = pipeline(HwAccel::Vaapi, Route::Download);
    let device = pipeline(HwAccel::Vaapi, Route::Device);

    assert!(cpu.on_the_cpu() && download.on_the_cpu());
    assert_eq!((cpu.slots(), download.slots()), (2, 2));
    assert!(!device.on_the_cpu());
    assert_eq!(device.slots(), 1);
}

#[test]
fn a_re_encode_with_no_tone_map_on_a_device_costs_one_slot_and_no_cpu() {
    let plain = Pipeline {
        tonemap: None,
        ..pipeline(HwAccel::Vaapi, Route::Cpu)
    };

    assert!(!plain.on_the_cpu());
    assert_eq!(plain.slots(), 1);
}

#[test]
fn a_software_encode_is_on_the_cpu_and_one_slot_unless_it_also_maps() {
    let plain = Pipeline {
        tonemap: None,
        ..pipeline(HwAccel::Software, Route::Cpu)
    };

    assert!(plain.on_the_cpu());
    assert_eq!(plain.slots(), 1);
    assert_eq!(pipeline(HwAccel::Software, Route::Cpu).slots(), 2);
}

#[test]
fn the_vectors_a_session_runs_are_the_ones_the_boot_probe_ran() {
    let probe = pipeline(HwAccel::Vaapi, Route::Download);

    assert_eq!(
        probe
            .video_filter(Some((320, 180)))
            .unwrap()
            .split(',')
            .next(),
        Some("scale_vaapi=w=320:h=180")
    );
}
