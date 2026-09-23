# dst-ktex

Don't Starve Together **KTEX** 纹理容器解析与解码（ktech 兼容），纯 Rust。

从 [dst-huiji-wiki](https://github.com/AshBarks/dst-huiji-wiki) 抽出的独立 crate，
被 wiki 图片管线与 `dst-anim-tool` 共用。

## 能力

- 容器解析：magic/位域头/mipmap 元数据；LE 端序优先 + fill 探测回退 BE；
  兼容 pre-caves 旧布局（bits 14..31 全 1）。
- 解码 mipmap 0：DXT1/3/5（`texpresso`，MIT）与未压缩 RGB/RGBA（按 pitch 行距）。
- 与 ktech 一致的输出语义：垂直翻转（UV 原点左下）+ a∈(0,1) 反预乘
  （`DecodeOptions::demultiply`，默认开）。
- `build_tex` / `compress_bc3`：生成合成 KTEX fixture，测试无需游戏素材。
- `trailing_pre_multiply_alpha`：读取可选的文件尾预乘标记。

## 快速开始

```rust
let bytes = std::fs::read("foo.tex")?;
let img = dst_ktex::decode_mipmap0(&bytes, dst_ktex::DecodeOptions::default())?;
img.save("foo.png")?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

## 兼容性

`DECODER_VERSION` 为 `"ktex-rs/1"`；dst-huiji-wiki 的图片历史 manifest 记录该值，
解码语义变更时必须同步 bump 以触发全量重处理。

## License

MIT
