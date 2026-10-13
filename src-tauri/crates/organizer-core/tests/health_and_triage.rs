use organizer_core::health::{
    check_magic_bytes, evaluate_file_health, find_recovery_pairs, MagicByteStatus,
};
use organizer_core::triage::{
    categorize_extension, resolve_collision_path, scan_and_plan_triage, TriageCategory,
    TriageOptions,
};
use std::fs::File;
use std::io::{Read, Write};
use tempfile::tempdir;

#[test]
fn test_magic_bytes_detection() {
    let dir = tempdir().unwrap();

    // Valid JPEG
    let jpeg_path = dir.path().join("photo.jpg");
    let mut f = File::create(&jpeg_path).unwrap();
    f.write_all(&[0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46]).unwrap();

    let status = check_magic_bytes(&jpeg_path, &[0xFF, 0xD8, 0xFF, 0xE0]);
    assert_eq!(
        status,
        MagicByteStatus::Valid {
            detected_format: "JPEG".into()
        }
    );

    // Mismatched PNG header with .jpg extension
    let status_mismatch = check_magic_bytes(&jpeg_path, &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]);
    assert_eq!(
        status_mismatch,
        MagicByteStatus::Mismatch {
            detected_format: "PNG".into(),
            expected_format: "jpg".into()
        }
    );

    // Corrupted header on jpeg
    let status_corrupt = check_magic_bytes(&jpeg_path, &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
    assert!(matches!(status_corrupt, MagicByteStatus::CorruptedHeader { .. }));
}

#[test]
fn test_evaluate_health_and_sector_holes() {
    let dir = tempdir().unwrap();

    // Intact PNG
    let good_png = dir.path().join("good.png");
    let mut f = File::create(&good_png).unwrap();
    f.write_all(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]).unwrap();
    // Fill with high entropy pseudo-data
    let data: Vec<u8> = (0..2048).map(|i| (i * 37 % 256) as u8).collect();
    f.write_all(&data).unwrap();
    drop(f);

    let report_good = evaluate_file_health(&good_png).unwrap();
    assert_eq!(
        report_good.magic_status,
        MagicByteStatus::Valid {
            detected_format: "PNG".into()
        }
    );
    assert_eq!(report_good.null_stats.sector_sized_null_blocks, 0);
    assert!(report_good.health_score >= 80);

    // Corrupted PNG with bad sector hole (1024 bytes of 0x00) and zeroed header
    let bad_png = dir.path().join("bad.png");
    let mut f = File::create(&bad_png).unwrap();
    let mut bad_data = vec![0u8; 2056];
    bad_data[1500..2000].copy_from_slice(&data[0..500]);
    f.write_all(&bad_data).unwrap();
    drop(f);

    let report_bad = evaluate_file_health(&bad_png).unwrap();
    assert!(matches!(
        report_bad.magic_status,
        MagicByteStatus::CorruptedHeader { .. }
    ));
    assert!(report_bad.null_stats.sector_sized_null_blocks >= 1);
    assert!(report_bad.health_score < 50);
}

#[test]
fn test_recovery_pair_comparison() {
    let dir = tempdir().unwrap();

    // Candidate A: Healthy JPEG
    let path_a = dir.path().join("family_vacation.jpg");
    let mut fa = File::create(&path_a).unwrap();
    fa.write_all(&[0xFF, 0xD8, 0xFF, 0xE0]).unwrap();
    let data: Vec<u8> = (0..4096).map(|i| (i * 73 % 256) as u8).collect();
    fa.write_all(&data).unwrap();
    drop(fa);

    // Candidate B: Damaged recovered copy from bad drive sectors
    let path_b = dir.path().join("family_vacation (recovered).jpg");
    let mut fb = File::create(&path_b).unwrap();
    let mut corrupted = vec![0u8; 4100];
    // Copy only partial body, header damaged
    corrupted[2000..3000].copy_from_slice(&data[0..1000]);
    fb.write_all(&corrupted).unwrap();
    drop(fb);

    let report_a = evaluate_file_health(&path_a).unwrap();
    let report_b = evaluate_file_health(&path_b).unwrap();

    let pairs = find_recovery_pairs(vec![report_a, report_b]);
    assert_eq!(pairs.len(), 1);
    let pair = &pairs[0];
    assert_eq!(pair.recommendation, organizer_core::health::HealthRecommendation::KeepA);
    assert!(pair.explanation.contains("Candidate A"));
}

#[test]
fn test_triage_categorization_and_planning() {
    let dir = tempdir().unwrap();

    // Create test files across categories
    let doc = dir.path().join("contract.pdf");
    File::create(&doc).unwrap();

    let img = dir.path().join("photo.png");
    File::create(&img).unwrap();

    let installer = dir.path().join("setup.exe");
    File::create(&installer).unwrap();

    let partial = dir.path().join("huge_game.iso.crdownload");
    File::create(&partial).unwrap();

    let atom = dir.path().join("mystery_unnamed_atom");
    File::create(&atom).unwrap();

    assert_eq!(categorize_extension("pdf"), TriageCategory::Documents);
    assert_eq!(categorize_extension("png"), TriageCategory::Images);
    assert_eq!(categorize_extension("exe"), TriageCategory::Installers);
    assert_eq!(categorize_extension("crdownload"), TriageCategory::IncompleteDownloads);
    assert_eq!(categorize_extension(""), TriageCategory::AtomOrOther);

    let options = TriageOptions::default();
    let plan = scan_and_plan_triage(dir.path(), &options).unwrap();

    assert_eq!(plan.items.len(), 5);
    assert!(plan.incomplete_downloads_count >= 1);
}

#[test]
fn test_collision_path_resolution() {
    let dir = tempdir().unwrap();
    let target = dir.path().join("document.pdf");
    File::create(&target).unwrap();

    let resolved = resolve_collision_path(&target);
    assert_eq!(
        resolved.file_name().unwrap().to_str().unwrap(),
        "document (1).pdf"
    );

    // Create the collision candidate as well
    File::create(&resolved).unwrap();
    let resolved2 = resolve_collision_path(&target);
    assert_eq!(
        resolved2.file_name().unwrap().to_str().unwrap(),
        "document (2).pdf"
    );
}

#[test]
fn test_extension_rectification() {
    let dir = tempdir().unwrap();

    // Create a JPEG file mislabeled as .bin
    let bin_photo = dir.path().join("file00183.bin");
    let mut f = File::create(&bin_photo).unwrap();
    f.write_all(&[0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46]).unwrap();
    f.write_all(&[0x12, 0x34, 0x56, 0x78]).unwrap();
    drop(f);

    let proposals = organizer_core::health::scan_for_extension_rectifications(dir.path()).unwrap();
    assert_eq!(proposals.len(), 1);
    assert_eq!(proposals[0].file_name, "file00183.bin");
    assert_eq!(proposals[0].detected_format, "JPEG");
    assert_eq!(proposals[0].suggested_extension, "jpg");

    // Execute rectification
    let items = vec![(
        proposals[0].current_path.clone(),
        proposals[0].suggested_new_path.clone(),
    )];
    let results = organizer_core::health::execute_extension_rectifications(items).unwrap();
    assert_eq!(results.len(), 1);
    assert!(results[0].success); // success

    assert!(!bin_photo.exists());
    assert!(dir.path().join("file00183.jpg").exists());
}

#[test]
fn test_perceptual_image_clustering() {
    let dir = tempdir().unwrap();

    // Image 1: Valid PNG header + sample gradient
    let img1 = dir.path().join("photo_original.png");
    let mut f1 = File::create(&img1).unwrap();
    // PNG signature + IHDR header chunk (width: 100, height: 100)
    let png_header: [u8; 24] = [
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // signature
        0x00, 0x00, 0x00, 0x0D, // IHDR length (13)
        0x49, 0x48, 0x44, 0x52, // IHDR
        0x00, 0x00, 0x00, 0x64, // width 100
        0x00, 0x00, 0x00, 0x64, // height 100
    ];
    f1.write_all(&png_header).unwrap();
    let body1: Vec<u8> = (0..1024).map(|i| (i % 256) as u8).collect();
    f1.write_all(&body1).unwrap();
    drop(f1);

    // Image 2: Near duplicate copy of Image 1 with minor difference in body
    let img2 = dir.path().join("photo_resized.png");
    let mut f2 = File::create(&img2).unwrap();
    let mut png_header_smaller = png_header;
    png_header_smaller[19] = 0x32; // width 50
    png_header_smaller[23] = 0x32; // height 50
    f2.write_all(&png_header_smaller).unwrap();
    let mut body2 = body1.clone();
    // Minor variation in a few bytes
    body2[500] = 200;
    f2.write_all(&body2).unwrap();
    drop(f2);

    let clusters = organizer_core::perceptual::scan_and_cluster_near_duplicates(dir.path(), 6).unwrap();
    assert_eq!(clusters.len(), 1);
    assert_eq!(clusters[0].members.len(), 2);
    // Highest resolution should be primary keep
    assert_eq!(clusters[0].primary_keep_path, img1.to_string_lossy().to_string());
    assert!(clusters[0].similarity_percentage >= 90.0);
}

#[test]
fn test_multi_copy_file_reconstruction() {
    let dir = tempdir().unwrap();

    // Ground truth: 8192 bytes (2 x 4096 chunks)
    let chunk1: Vec<u8> = (0..4096).map(|i| (i % 255 + 1) as u8).collect();
    let chunk2: Vec<u8> = (0..4096).map(|i| ((i * 3) % 255 + 1) as u8).collect();

    // Copy A: Chunk 1 healthy, Chunk 2 zero-hole
    let path_a = dir.path().join("document_copy_a.bin");
    let mut fa = File::create(&path_a).unwrap();
    fa.write_all(&chunk1).unwrap();
    fa.write_all(&vec![0u8; 4096]).unwrap();
    drop(fa);

    // Copy B: Chunk 1 zero-hole, Chunk 2 healthy
    let path_b = dir.path().join("document_copy_b.bin");
    let mut fb = File::create(&path_b).unwrap();
    fb.write_all(&vec![0u8; 4096]).unwrap();
    fb.write_all(&chunk2).unwrap();
    drop(fb);

    // Plan reconstruction
    let plan = organizer_core::reconstruction::analyze_reconstruction_plan(&path_a, &path_b).unwrap();
    assert_eq!(plan.block_map.total_chunks, 2);
    assert_eq!(plan.block_map.chunks_from_a, 1);
    assert_eq!(plan.block_map.chunks_from_b, 1);
    assert_eq!(plan.block_map.unrecoverable_chunks, 0);
    assert!(plan.can_achieve_full_repair);
    assert_eq!(plan.potential_recovery_percentage, 100.0);

    // Execute reconstruction
    let res = organizer_core::reconstruction::execute_reconstruction(&plan).unwrap();
    assert_eq!(res.bytes_written, 8192);
    assert_eq!(res.remaining_null_holes, 0);

    // Verify reconstructed file matches ground-truth bytes
    let mut reconstructed_data = Vec::new();
    let mut fr = File::open(&res.output_path).unwrap();
    fr.read_to_end(&mut reconstructed_data).unwrap();
    assert_eq!(&reconstructed_data[..4096], &chunk1[..]);
    assert_eq!(&reconstructed_data[4096..], &chunk2[..]);
}

#[test]
fn test_privacy_shield_filtering() {
    let dir = tempdir().unwrap();

    // Create normal file
    let normal_file = dir.path().join("vacation_photos.zip");
    File::create(&normal_file).unwrap();

    // Create sensitive files (.ssh folder + private key, .kdbx database, .env file)
    let ssh_dir = dir.path().join(".ssh");
    std::fs::create_dir_all(&ssh_dir).unwrap();
    let id_rsa = ssh_dir.join("id_rsa");
    File::create(&id_rsa).unwrap();

    let kdbx_file = dir.path().join("passwords.kdbx");
    File::create(&kdbx_file).unwrap();

    let env_file = dir.path().join(".env");
    File::create(&env_file).unwrap();

    // 1. Verify triage skips sensitive files
    let options = TriageOptions::default();
    let triage_plan = scan_and_plan_triage(dir.path(), &options).unwrap();
    for item in &triage_plan.items {
        assert_ne!(item.file_name, "passwords.kdbx");
        assert_ne!(item.file_name, ".env");
        assert_ne!(item.file_name, "id_rsa");
    }

    // 2. Verify health rectification skips sensitive keys
    let rect_proposals = organizer_core::health::scan_for_extension_rectifications(dir.path()).unwrap();
    for prop in &rect_proposals {
        assert_ne!(prop.file_name, "id_rsa");
        assert_ne!(prop.file_name, "passwords.kdbx");
    }
}
