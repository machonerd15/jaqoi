mod encoder;
mod decoder;

pub enum Channels {
    RGB,
    RGBA
}

pub enum Colorspace {
    SrgbLinearAlpha,
    AllLinearAlpha
}

pub struct ImgMetadata {
    pub width: u32,
    pub height: u32,
    pub channels: Channels,
    pub colorspace: Colorspace
}


pub fn encode(rgb_pixels: &Vec<u8>, metadata: &ImgMetadata) -> Vec<u8> {
    let mut raw_bytes: Vec<u8> = Vec::new();

    encoder::add_header(&mut raw_bytes, &metadata);

    let alpha_included = match metadata.channels {
        Channels::RGB => {false}
        Channels::RGBA => {true}
    };
    encoder::add_chunks(&mut raw_bytes, &rgb_pixels, alpha_included).unwrap();

    encoder::add_end_marker(&mut raw_bytes);

    raw_bytes
}


pub fn decode(_raw_file_bytes: Vec<u8>) -> (ImgMetadata, Vec<u8>) {
    (ImgMetadata{
        width: 0,
        height: 0,
        channels: Channels::RGB,
        colorspace: Colorspace::SrgbLinearAlpha,
    }, vec!(0))

}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use image;

    use super::*;

    fn create_random_image(width: u32, height: u32) -> image::RgbImage {
        let mut img = image::RgbImage::new(width,height);
        for x in 0..width {
           for y in 0..height {
               img.put_pixel(x, y, image::Rgb([(x % 256) as u8, (y % 256) as u8, (x+y % 256) as u8]));
           }
        }
        img
    }

    #[test]
    fn test_encode() {
        let source_image = create_random_image(2, 2);

        let metadata = ImgMetadata {
            width: 2,
            height: 2,
            channels: Channels::RGB,
            colorspace: Colorspace::SrgbLinearAlpha,
        };

        let qoi = encode(source_image.as_raw(), &metadata);

        let mut reader = image::io::Reader::new(Cursor::new(qoi));
        reader.set_format(image::ImageFormat::Qoi);
        let output_image = reader.decode().expect("Decode should be successful");
        let output_image = output_image.as_rgb8().expect("Should be rgb8");
        assert!(source_image.eq(output_image));
    }
}
