use std::collections::HashMap;

use cottontail::{
    core::read_file_whole,
    image::{Bitmap, Color},
    math::{Random, Vec2i},
};

fn main() {
    let top_left = Vec2i::new(37, 284);
    let bottom_right = Vec2i::new(763, 1152);
    let font_size = 100.0;
    let font_data = read_file_whole("resources/OMEGLE.ttf").unwrap();
    let mut background = Bitmap::from_png_file_or_panic("resources/Bingo_card.png");
    let color = Color::new(0.6, 0.3, 0.4, 1.0);

    let digits = "0123456789";
    let font = fontdue::Font::from_bytes(font_data, fontdue::FontSettings::default()).unwrap();
    let digits_metrics_bitmaps_premultiplied: HashMap<char, _> = digits
        .chars()
        .map(|digit| (digit, font.rasterize(digit, font_size)))
        .map(|(digit, (metrics, bytes))| {
            let mut bitmap_premultiplied = Bitmap::from_greyscale_bytes_premultiplied(
                &bytes,
                metrics.width as u32,
                metrics.height as u32,
            );
            for pixel in bitmap_premultiplied.data.iter_mut() {
                pixel.r = ((pixel.r as f32) * color.r) as u8;
                pixel.g = ((pixel.g as f32) * color.g) as u8;
                pixel.b = ((pixel.b as f32) * color.b) as u8;
            }
            (digit, (metrics, bitmap_premultiplied))
        })
        .collect();

    for (digit, (_metrics, bitmap_premultiplied)) in digits_metrics_bitmaps_premultiplied.iter() {
        bitmap_premultiplied
            .to_unpremultiplied_alpha()
            .write_to_png_file(&format!("test/{}.png", digit));
    }

    let mut number_bitmaps = HashMap::new();
    for number in 1..=75 {
        let number_string = number.to_string();
        let mut layout =
            fontdue::layout::Layout::new(fontdue::layout::CoordinateSystem::PositiveYDown);
        layout.append(
            &[&font],
            &fontdue::layout::TextStyle::new(&number_string, font_size, 0),
        );
        let glyphs = layout.glyphs().clone();

        let x_min = glyphs
            .iter()
            .fold(std::f32::MAX, |acc, glyph_pos| f32::min(acc, glyph_pos.x));
        let y_min = glyphs
            .iter()
            .fold(std::f32::MAX, |acc, glyph_pos| f32::min(acc, glyph_pos.y));
        let offset_x = if x_min < 0.0 { -x_min } else { 0.0 };
        let offset_y = if y_min < 0.0 { -y_min } else { 0.0 };
        let x_max = offset_x
            + glyphs.iter().fold(std::f32::MIN, |acc, glyph_pos| {
                f32::max(acc, glyph_pos.x + glyph_pos.width as f32)
            });
        let y_max = offset_y
            + glyphs.iter().fold(std::f32::MIN, |acc, glyph_pos| {
                f32::max(acc, glyph_pos.y + glyph_pos.height as f32)
            });

        let mut number_bitmap_premultiplied = Bitmap::new(x_max.ceil() as u32, y_max.ceil() as u32);
        for glyphpos in glyphs.iter() {
            let digit = glyphpos.key.c;
            let (_digit_metrics, digit_bitmap_premultiplied) =
                digits_metrics_bitmaps_premultiplied.get(&digit).unwrap();
            digit_bitmap_premultiplied.blit_to_alpha_blended_premultiplied(
                &mut number_bitmap_premultiplied,
                Vec2i::new(
                    (offset_x + glyphpos.x.round()) as i32,
                    (offset_y + glyphpos.y.round()) as i32,
                ),
                true,
                cottontail::image::ColorBlendMode::Normal,
            );

            // println!("{:#?}", &glyphpos);
            // println!("{:#?}", &digit_metrics);
        }

        number_bitmap_premultiplied
            .to_unpremultiplied_alpha()
            .write_to_png_file(&format!("test_numbers/{}.png", number));

        number_bitmaps.insert(number, number_bitmap_premultiplied);
    }

    let cell_width = (bottom_right.x - top_left.x) / 5;
    let cell_height = (bottom_right.y - top_left.y) / 5;

    let mut random = Random::new_from_seed(1234);
    for y in 0..5 {
        for x in 0..5 {
            if x == 2 && y == 2 {
                continue;
            }
            let center = top_left
                + Vec2i::new(
                    x * cell_width + cell_width / 2,
                    y * cell_height + cell_height / 2,
                );

            let number = random.u32_in_range(1, 75);
            let number_bitmap = number_bitmaps.get(&number).unwrap();
            number_bitmap.blit_to_alpha_blended_premultiplied(
                &mut background,
                center - number_bitmap.rect().dim / 2,
                true,
                cottontail::image::ColorBlendMode::Multiply,
            );
        }
    }

    background
        .to_unpremultiplied_alpha()
        .write_to_png_file("output.png");
}
