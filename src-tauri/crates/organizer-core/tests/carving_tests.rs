use organizer_core::carving::{
    generate_canonical_name, resolve_collision_safe_path, scan_carving_proposals,
    execute_carve_renames, sniff_metadata, parse_jpeg_exif, parse_id3v2, parse_pdf_metadata,
};
use std::fs::File;
use std::io::Write;
use tempfile::tempdir;

#[test]
fn test_sniff_png_magic_bytes() {
    let header = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D];
    let meta = sniff_metadata(&header).expect("should identify PNG");
    assert_eq!(meta.extension, "png");
    assert_eq!(meta.mime, "image/png");
}

#[test]
fn test_sniff_pdf_metadata() {
    let pdf_header = b"%PDF-1.4\n1 0 obj\n<< /Title (Annual Financial Report) /CreationDate (D:20230514120000) >>\nendobj";
    let (date, title) = parse_pdf_metadata(pdf_header);
    assert_eq!(date, Some("2023-05-14".to_string()));
    assert_eq!(title, Some("Annual Financial Report".to_string()));

    let meta = sniff_metadata(pdf_header).expect("should identify PDF");
    assert_eq!(meta.extension, "pdf");
    assert_eq!(meta.date, Some("2023-05-14".to_string()));
    assert_eq!(meta.title, Some("Annual Financial Report".to_string()));

    let canonical = generate_canonical_name(&meta, "recovered_001", "a1b2c3d4");
    assert_eq!(canonical, "Annual Financial Report_2023-05-14.pdf");
}

#[test]
fn test_sniff_id3v2_mp3() {
    // Construct synthetic ID3v2.3 tag
    let mut data = Vec::new();
    data.extend_from_slice(b"ID3");
    data.push(3); // v2.3
    data.push(0); // revision
    data.push(0); // flags

    // Frames payload
    let mut frames = Vec::new();
    // TIT2 (Title)
    frames.extend_from_slice(b"TIT2");
    frames.extend_from_slice(&[0, 0, 0, 11]); // size 11
    frames.extend_from_slice(&[0, 0]); // flags
    frames.push(0); // encoding ISO-8859-1
    frames.extend_from_slice(b"Song Title");

    // TPE1 (Artist)
    frames.extend_from_slice(b"TPE1");
    frames.extend_from_slice(&[0, 0, 0, 13]); // size 13 (1 encoding + 12 chars)
    frames.extend_from_slice(&[0, 0]); // flags
    frames.push(0); // encoding
    frames.extend_from_slice(b"Great Artist");

    // ID3 syncsafe size
    let tag_len = frames.len();
    data.push(((tag_len >> 21) & 0x7F) as u8);
    data.push(((tag_len >> 14) & 0x7F) as u8);
    data.push(((tag_len >> 7) & 0x7F) as u8);
    data.push((tag_len & 0x7F) as u8);
    data.extend(frames);

    let (artist, title, _) = parse_id3v2(&data);
    assert_eq!(artist, Some("Great Artist".to_string()));
    assert_eq!(title, Some("Song Title".to_string()));

    let meta = sniff_metadata(&data).expect("should identify MP3");
    assert_eq!(meta.extension, "mp3");
    let canonical = generate_canonical_name(&meta, "f0042", "e5f6a7b8");
    assert_eq!(canonical, "Great Artist - Song Title.mp3");
}

#[test]
fn test_jpeg_exif_little_endian() {
    // Construct minimal JPEG with APP1 EXIF segment (Little Endian II)
    let mut buf = Vec::new();
    buf.extend_from_slice(&[0xFF, 0xD8]); // SOI

    // APP1 marker
    let mut app1 = Vec::new();
    app1.extend_from_slice(b"Exif\0\0");

    // TIFF Header: "II", magic 42, offset to IFD0 (8)
    let mut tiff = Vec::new();
    tiff.extend_from_slice(b"II");
    tiff.extend_from_slice(&42u16.to_le_bytes());
    tiff.extend_from_slice(&8u32.to_le_bytes()); // IFD0 starts at byte 8

    // IFD0: 2 entries (Model 0x0110, DateTime 0x0132)
    tiff.extend_from_slice(&2u16.to_le_bytes());

    // Entry 1: Model (0x0110), Type ASCII (2), Count 13, Offset to string
    let model_str = b"iPhone 15 Pro\0";
    let model_offset = 8 + 2 + 12 + 12 + 4; // after IFD0 & next IFD pointer
    tiff.extend_from_slice(&0x0110u16.to_le_bytes());
    tiff.extend_from_slice(&2u16.to_le_bytes());
    tiff.extend_from_slice(&(model_str.len() as u32).to_le_bytes());
    tiff.extend_from_slice(&(model_offset as u32).to_le_bytes());

    // Entry 2: DateTime (0x0132), Type ASCII (2), Count 20, Offset
    let date_str = b"2023:09:22 15:30:00\0";
    let date_offset = model_offset + model_str.len();
    tiff.extend_from_slice(&0x0132u16.to_le_bytes());
    tiff.extend_from_slice(&2u16.to_le_bytes());
    tiff.extend_from_slice(&(date_str.len() as u32).to_le_bytes());
    tiff.extend_from_slice(&(date_offset as u32).to_le_bytes());

    // Next IFD offset = 0
    tiff.extend_from_slice(&0u32.to_le_bytes());

    // String payloads
    tiff.extend_from_slice(model_str);
    tiff.extend_from_slice(date_str);

    app1.extend(tiff);

    let app1_len = app1.len() + 2;
    buf.extend_from_slice(&[0xFF, 0xE1]);
    buf.push((app1_len >> 8) as u8);
    buf.push((app1_len & 0xFF) as u8);
    buf.extend(app1);

    let (date, model) = parse_jpeg_exif(&buf);
    assert_eq!(date, Some("2023-09-22_15-30-00".to_string()));
    assert_eq!(model, Some("iPhone 15 Pro".to_string()));

    let meta = sniff_metadata(&buf).expect("should sniff JPEG with EXIF");
    let name = generate_canonical_name(&meta, "f00099.bin", "aabbccdd");
    assert_eq!(name, "IMG_2023-09-22_15-30-00_iPhone 15 Pro.jpg");
}

#[test]
fn test_scan_and_execute_carve_renames_with_collision_avoidance() {
    let dir = tempdir().unwrap();

    // Write orphan PNG file named f00123.bin
    let bin_path = dir.path().join("f00123.bin");
    let mut f = File::create(&bin_path).unwrap();
    f.write_all(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x01, 0x02, 0x03]).unwrap();
    drop(f);

    let proposals = scan_carving_proposals(dir.path()).expect("scan proposals");
    assert_eq!(proposals.len(), 1);
    assert_eq!(proposals[0].detected_extension, "png");
    assert_eq!(proposals[0].proposed_name, "RECOVERED_0000000c.png");

    // Execute rename
    let items = vec![(proposals[0].source_path.clone(), proposals[0].proposed_path.clone())];
    let res = execute_carve_renames(&items);
    assert_eq!(res.succeeded, 1);
    assert_eq!(res.failed, 0);

    assert!(!bin_path.exists());
    let target = dir.path().join("RECOVERED_0000000c.png");
    assert!(target.exists());

    // Test collision avoidance when same name target exists
    let safe = resolve_collision_safe_path(dir.path(), "RECOVERED_0000000c.png");
    assert_eq!(safe.file_name().unwrap().to_str().unwrap(), "RECOVERED_0000000c_1.png");
}
