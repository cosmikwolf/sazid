use ratatui::style::Color;

// Compute both rainbow color and its hue-inverted counterpart.
pub fn get_rainbow_and_inverse_colors(
  step: u32,
  total_steps: u32,
) -> (Color, Color) {
  let hue = step as f32 / total_steps as f32;
  let inverse_hue = (hue + 0.5) % 1.0; // Add 0.5 to invert the hue in the spectrum and use modulo to wrap around

  let rainbow_color = hsv_to_rgb(hue, 1.0, 1.0);
  let inverse_color = hsv_to_rgb(inverse_hue, 1.0, 1.0);

  // Map the floating point RGB values to 8-bit integers
  let rainbow_rgb = (
    (rainbow_color.0 * 255.0).round() as u8,
    (rainbow_color.1 * 255.0).round() as u8,
    (rainbow_color.2 * 255.0).round() as u8,
  );

  let inverse_rgb = (
    (inverse_color.0 * 255.0).round() as u8,
    (inverse_color.1 * 255.0).round() as u8,
    (inverse_color.2 * 255.0).round() as u8,
  );

  (
    Color::Rgb(rainbow_rgb.0, rainbow_rgb.1, rainbow_rgb.2),
    Color::Rgb(inverse_rgb.0, inverse_rgb.1, inverse_rgb.2),
  )
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
  let i = (h * 6.0).floor() as i32;
  let f = h * 6.0 - i as f32;
  let p = v * (1.0 - s);
  let q = v * (1.0 - f * s);
  let t = v * (1.0 - (1.0 - f) * s);

  match i % 6 {
    0 => (v, t, p),
    1 => (q, v, p),
    2 => (p, v, t),
    3 => (p, q, v),
    4 => (t, p, v),
    _ => (v, p, q),
  }
}
