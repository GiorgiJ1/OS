use enigo::{Coordinate, Enigo, Mouse, Settings};
use opencv::{
    core::{self, Point, Scalar, Size},
    imgproc,
    prelude::*,
    videoio,
};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    // 1. Initialize Enigo for mouse automation
    let mut enigo = Enigo::new(&Settings::default())?;
    let (screen_w, screen_h) = enigo.main_display()?;
    println!("Screen Size: {}x{}", screen_w, screen_h);

    // 2. Open the default webcam (camera 0)
    let mut cam = videoio::VideoCapture::new(0, videoio::CAP_ANY)?;
    if !videoio::VideoCapture::is_opened(&cam)? {
        return Err("Could not open the webcam.".into());
    }

    // Define window for debugging/visualization
    let window_name = "Hand Mouse Tracker";
    opencv::highgui::named_window(window_name, opencv::highgui::WINDOW_AUTOSIZE)?;

    let mut frame = Mat::default();
    let mut hsv = Mat::default();
    let mut mask = Mat::default();

    // 3. Define color ranges in HSV space to track
    // (Adjust these values to isolate your skin tone or a colored item/glove)
    // The current range is optimized for skin-tone/orange-ish tones.
    let lower_bound = Scalar::new(0.0, 30.0, 60.0, 0.0);
    let upper_bound = Scalar::new(20.0, 150.0, 255.0, 0.0);

    println!("Tracking started. Press 'q' in the window to exit.");

    loop {
        // Read current frame from camera
        if !cam.read(&mut frame)? || frame.empty() {
            continue;
        }

        // Flip frame horizontally so it acts like a mirror
        let mut flipped_frame = Mat::default();
        core::flip(&frame, &mut flipped_frame, 1)?;
        frame = flipped_frame;

        let frame_w = frame.cols();
        let frame_h = frame.rows();

        // Convert the image to HSV color space
        imgproc::cvt_color(&frame, &mut hsv, imgproc::COLOR_BGR2HSV, 0)?;

        // Filter out pixels not in our target color range
        core::in_range(&lower_bound, &upper_bound, &mask)?;

        // Reduce tracking noise using morphology operations
        let kernel = imgproc::get_structuring_element(
            imgproc::MORPH_RECT,
            Size::new(5, 5),
            Point::new(-1, -1),
        )?;
        imgproc::erode(&mask, &mut mask, &kernel, Point::new(-1, -1), 1, core::BORDER_CONSTANT, imgproc::morphology_default_border_value()?)?;
        imgproc::dilate(&mask, &mut mask, &kernel, Point::new(-1, -1), 1, core::BORDER_CONSTANT, imgproc::morphology_default_border_value()?)?;

        // Find contours/shapes of the filtered regions
        let mut contours = core::Vector::<core::Vector<Point>>::new();
        imgproc::find_contours(
            &mask,
            &mut contours,
            imgproc::RETR_EXTERNAL,
            imgproc::CHAIN_APPROX_SIMPLE,
            Point::new(0, 0),
        )?;

        // Identify the largest shape (assumed to be your hand or object)
        let mut max_area = 0.0;
        let mut target_contour_idx: Option<usize> = None;

        for (i, contour) in contours.iter().enumerate() {
            let area = imgproc::contour_area(&contour, false)?;
            if area > max_area && area > 500.0 { // Minimum threshold to avoid tracking single pixels
                max_area = area;
                target_contour_idx = Some(i);
            }
        }

        // Move the mouse if a valid tracking object is detected
        if let Some(idx) = target_contour_idx {
            let target_contour = contours.get(idx)?;
            
            // Draw a visual box around the detected hand/object
            let bounding_rect = imgproc::bounding_rect(&target_contour)?;
            imgproc::rectangle(&mut frame, bounding_rect, Scalar::new(0.0, 255.0, 0.0, 0.0), 2, imgproc::LINE_8, 0)?;

            // Find the center (centroid) of the bounding box
            let cx = bounding_rect.x + bounding_rect.width / 2;
            let cy = bounding_rect.y + bounding_rect.height / 2;

            // Map camera coordinates cleanly to screen dimensions
            let target_x = (cx as f32 / frame_w as f32) * screen_w as f32;
            let target_y = (cy as f32 / frame_h as f32) * screen_h as f32;

            // Update mouse position natively
            let _ = enigo.move_mouse(target_x as i32, target_y as i32, Coordinate::Absolute);
        }

        // Show window feed
        opencv::highgui::imshow(window_name, &frame)?;

        // Break loop when 'q' is pressed (delay of 10ms for processing overhead)
        let key = opencv::highgui::wait_key(10)?;
        if key == 'q' as i32 {
            break;
        }
    }

    Ok(())
}