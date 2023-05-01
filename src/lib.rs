mod encoder;
mod decoder;

#[derive(Eq, PartialEq, Debug)]
pub enum Channels {
    RGB,
    RGBA
}

#[derive(Eq, PartialEq, Debug)]
pub enum Colorspace {
    SrgbLinearAlpha,
    AllLinearAlpha
}

#[derive(Eq, PartialEq, Debug)]
pub struct ImgMetadata {
    pub width: u32,
    pub height: u32,
    pub channels: Channels,
    pub colorspace: Colorspace
}

#[derive(Eq, PartialEq, Debug)]
enum Operation {
    QoiOpRgb,
    QoiOpRgba,
    QoiOpIndex,
    QoiOpDiff,
    QoiOpLuma,
    QoiOpRun,
}

const QOI_OP_RGB: u8 = 0b11111110;
const QOI_OP_RGBA: u8 = 0b11111111;
const QOI_OP_INDEX: u8 = 0b00;
const QOI_OP_DIFF: u8 = 0b01;
const QOI_OP_LUMA: u8 = 0b10;
const QOI_OP_RUN: u8 = 0b11;

//todo - make private again once index calculation is moved
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct Pixel {
    r: u8,
    g: u8,
    b: u8,
    a:u8,
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


pub fn decode(raw_file_bytes: &Vec<u8>) -> (ImgMetadata, Vec<u8>) {
    return decoder::decode(&raw_file_bytes);
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

    #[test]
    fn test_decode() {

    }
}
