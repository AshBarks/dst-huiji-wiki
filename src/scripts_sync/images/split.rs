//! atlas 切割：解析 ktools 布局 XML，按 UV 坐标从解码图裁出单张 sprite。
//!
//! 坐标移植自旧 Python `ktech_split`（update_scripts.py:162-173），必须保持
//! 逐位一致：
//! - `int(float(u) * dim)` 向零截断 → `f64 as i64`；
//! - 裁剪框 `(u1, H - v2 - 1, u2 + 1, H - v1)`：atlas UV 原点在左下、图像
//!   坐标原点在左上，v 轴翻转；
//! - 输出目录名去除**一个**尾数字（`foo2.xml` → `foo/`）。
//!
//! 与 Python 的差异：PIL 的 `crop` 允许越界并补黑，这里将越界坐标钳制到图
//! 内（`u2 = 1.0` 的满宽 atlas 会少一圈黑边），空区域跳过并计数。

use crate::error::{Error, Result};
use image::DynamicImage;

/// 一个 sprite 区域（像素坐标，已含 v 轴翻转）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpriteRegion {
    /// Element 的 name 属性（`.tex` 已替换为 `.png`）。
    pub name: String,
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

/// 解析 atlas XML，返回按文档顺序的 sprite 区域列表。
///
/// 结构约定（与旧 Python 一致）：根节点下两层内的 `Element` 元素，
/// 属性 `name/u1/v1/u2/v2`。缺失属性或非法数值的 Element 跳过。
pub fn parse_atlas_regions(xml: &str, image_w: u32, image_h: u32) -> Result<Vec<SpriteRegion>> {
    let doc = roxmltree::Document::parse(xml)
        .map_err(|e| Error::ParseError(format!("atlas XML 解析失败: {e}")))?;
    let root = doc.root_element();
    let mut regions = Vec::new();
    for level1 in root.children() {
        if !level1.is_element() {
            continue;
        }
        for elem in level1.children() {
            if !elem.is_element() || elem.tag_name().name() != "Element" {
                continue;
            }
            if let Some(region) = parse_element(&elem, image_w, image_h) {
                regions.push(region);
            }
        }
    }
    Ok(regions)
}

fn parse_element(elem: &roxmltree::Node, image_w: u32, image_h: u32) -> Option<SpriteRegion> {
    let attr = |name: &str| elem.attribute(name);
    let name = attr("name")?.replace(".tex", ".png");
    let (u1, v1, u2, v2) = match (
        attr("u1").and_then(|v| v.parse::<f64>().ok()),
        attr("v1").and_then(|v| v.parse::<f64>().ok()),
        attr("u2").and_then(|v| v.parse::<f64>().ok()),
        attr("v2").and_then(|v| v.parse::<f64>().ok()),
    ) {
        (Some(a), Some(b), Some(c), Some(d)) => (a, b, c, d),
        _ => return None,
    };
    // int(float(u) * dim)：向零截断，与 Python `int()` 一致。
    let u1 = (u1 * f64::from(image_w)) as i64;
    let u2 = (u2 * f64::from(image_w)) as i64;
    let v1 = (v1 * f64::from(image_h)) as i64;
    let v2 = (v2 * f64::from(image_h)) as i64;
    let height = i64::from(image_h);
    // 裁剪框（左开右闭 → w = u2-u1+1），v 轴翻转。
    let left = u1;
    let top = height - v2 - 1;
    let right = u2 + 1;
    let bottom = height - v1;
    // 钳制到图内（PIL 越界补黑的差异见模块文档）。
    let left = left.clamp(0, i64::from(image_w));
    let right = right.clamp(0, i64::from(image_w));
    let top = top.clamp(0, height);
    let bottom = bottom.clamp(0, height);
    let (w, h) = (right - left, bottom - top);
    if w <= 0 || h <= 0 {
        return None;
    }
    Some(SpriteRegion {
        name,
        x: left as u32,
        y: top as u32,
        w: w as u32,
        h: h as u32,
    })
}

/// 按区域裁剪（调用方保证区域在图内）。
pub fn crop(image: &DynamicImage, region: &SpriteRegion) -> DynamicImage {
    image.crop_imm(region.x, region.y, region.w, region.h)
}

/// 输出目录名：xml base 去除一个尾数字（`foo2` → `foo`，`foo12` → `foo1`）。
pub fn split_dir_name(xml_base: &str) -> String {
    match xml_base.chars().last() {
        Some(c) if c.is_ascii_digit() => xml_base[..xml_base.len() - 1].to_string(),
        _ => xml_base.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::GenericImageView as _;
    use image::{Rgba, RgbaImage};

    const SAMPLE_XML: &str = r#"<?xml version="1.0"?>
    <Atlas>
        <Elements>
            <Element name="a.tex" u1="0" v1="0" u2="0.5" v2="0.5"/>
            <Element name="b.tex" u1="0.5" v1="0.5" u2="1" v2="1"/>
            <Element name="bad.tex"/>  <!-- 缺 UV 属性，跳过 -->
            <!-- u1==u2 且 v1==v2：inclusive 语义下是 1x1 的最小区域 -->
            <Element name="tiny.tex" u1="0.9" v1="0.9" u2="0.9" v2="0.9"/>
            <Element name="inverted.tex" u1="0.9" v1="0.9" u2="0.1" v2="0.1"/>
        </Elements>
        <Other>
            <NotElement name="c.tex" u1="0" v1="0" u2="1" v2="1"/>
        </Other>
    </Atlas>"#;

    #[test]
    fn test_parse_regions_and_v_flip() {
        // 4x4 图：u/v 归一化 → 像素。
        let regions = parse_atlas_regions(SAMPLE_XML, 4, 4).unwrap();
        // a: u1=0,v1=0,u2=0.5*4=2,v2=2 → left=0, top=4-2-1=1, right=3, bottom=4
        assert_eq!(
            regions[0],
            SpriteRegion {
                name: "a.png".into(),
                x: 0,
                y: 1,
                w: 3,
                h: 3
            }
        );
        // b: u1=2,u2=4→right=5 钳制为 4; v1=2,v2=4→top=-1 钳制为 0, bottom=2
        assert_eq!(
            regions[1],
            SpriteRegion {
                name: "b.png".into(),
                x: 2,
                y: 0,
                w: 2,
                h: 2
            }
        );
        // tiny: u1=u2=3, v1=v2=3 → left=3,right=4,top=4-3-1=0,bottom=1
        assert_eq!(
            regions[2],
            SpriteRegion {
                name: "tiny.png".into(),
                x: 3,
                y: 0,
                w: 1,
                h: 1
            }
        );
        // inverted（w/h 为负）、bad（缺属性）与非 Element 均被跳过
        assert_eq!(regions.len(), 3);
    }

    #[test]
    fn test_crop_v_axis_semantics() {
        // 4x4 图，每行一色：0 白 1 灰 2 黑 3 蓝。
        // 取 v1=0.25, v2=0.5（UV 下半带，含 1px 入上边界，见 inclusive 语义）。
        let mut img = RgbaImage::new(4, 4);
        for x in 0..4 {
            img.put_pixel(x, 0, Rgba([255, 255, 255, 255]));
            img.put_pixel(x, 1, Rgba([128, 128, 128, 255]));
            img.put_pixel(x, 2, Rgba([0, 0, 0, 255]));
            img.put_pixel(x, 3, Rgba([0, 0, 255, 255]));
        }
        let dyn_img = DynamicImage::ImageRgba8(img);
        let regions = parse_atlas_regions(
            r#"<A><E><Element name="s.tex" u1="0" v1="0.25" u2="1" v2="0.5"/></E></A>"#,
            4,
            4,
        )
        .unwrap();
        // v1=1, v2=2 → top = 4-2-1 = 1, bottom = 4-1 = 3；u2=1.0 → right 钳制 4。
        assert_eq!(
            regions[0],
            SpriteRegion {
                name: "s.png".into(),
                x: 0,
                y: 1,
                w: 4,
                h: 2
            }
        );
        let cropped = crop(&dyn_img, &regions[0]);
        // 裁出图像第 1、2 行（灰、黑）——UV 原点在左下。
        assert_eq!(cropped.dimensions(), (4, 2));
        assert_eq!(cropped.get_pixel(0, 0), Rgba([128, 128, 128, 255]));
        assert_eq!(cropped.get_pixel(0, 1), Rgba([0, 0, 0, 255]));
    }

    #[test]
    fn test_split_dir_name() {
        assert_eq!(split_dir_name("foo2"), "foo");
        assert_eq!(split_dir_name("foo12"), "foo1");
        assert_eq!(split_dir_name("foo"), "foo");
        assert_eq!(split_dir_name("inventoryimages"), "inventoryimages");
    }

    #[test]
    fn test_parse_invalid_xml_errors() {
        assert!(parse_atlas_regions("<not-closed", 4, 4).is_err());
    }
}
