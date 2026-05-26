# DST 动画工具 - 数据处理流程分析

> 来源：[dont-starve-anim-tool](https://dont-starve-anim-tool.pages.dev/)
> 本地源码备份：`refs/dst-anim-tool-src/`

---

## 一、项目文件结构

| 文件                      | 作用           | 说明                                                                                 |
| ------------------------- | -------------- | ------------------------------------------------------------------------------------ |
| `index.html`              | 页面入口       | 加载两个 JS 模块和两个 CSS                                                           |
| `main-BcHBP8aN.js`        | **主业务逻辑** | 文件上传分发、anim/build 二进制解析、动画渲染、UI 交互                               |
| `Navigation-QA1c6I0M.js`  | **基础库**     | BinaryDataReader/Writer、Ktex 纹理解码、图像变换(crop/resize/paste/transform)、JSZip |
| `main-CBWRJZs9.css`       | 主样式         | UI 布局样式                                                                          |
| `Navigation-B64Ff65h.css` | 导航样式       | 导航栏样式                                                                           |

### 两个 JS 文件的职责划分

```
Navigation-QA1c6I0M.js (基础库层)
├── BinaryDataReader    — 二进制数据读取器
├── BinaryDataWriter    — 二进制数据写入器
├── Ktex / KtexHeader   — KTEX 纹理格式解码/编码
├── KtexMipmap          — Mipmap 数据
├── Specifications      — KTEX 规格定义(PreCave/PostCave)
├── Platform / PixelFormat / TextureType — 枚举定义
├── transform()         — 仿射变换图像
├── crop()              — 图像裁剪
├── resize()            — 图像缩放
├── paste()             — 图像粘贴
├── newCanvas()         — 创建 Canvas
├── preMultiplyAlpha()  — 预乘 Alpha
├── flipY()             — Y轴翻转
├── JSZip               — ZIP 压缩/解压
├── asyncLoadFile()     — 异步文件加载
├── loadImage()         — 图片加载
└── downloadFile()      — 文件下载

main-BcHBP8aN.js (业务逻辑层)
├── Vi()                — 文件上传入口，按扩展名分发
├── Hs()                — 二进制文件路由(读魔数 → wc/Cc)
├── js()                — JSON 文件路由(读 type → parseJson)
├── wc()                — anim.bin 解析器
├── Cc()                — build.bin 解析器
├── Cf()                — ZIP 包处理
├── _f()                — 标准 DST ZIP 处理
├── wf()                — .dyn 加密包处理
├── vr()                — XOR 解密/加密
├── oa()                — Build 数据设置 + splitAtlas
├── la()                — Anim 数据设置
├── splitAtlas()        — Atlas 纹理裁剪拼装为精灵图
├── Ci()                — 动画帧渲染(计算边界)
├── mr()                — Symbol Frame 查找
├── Ne()                — DST 字符串哈希函数
├── AnimElement (rs)    — 动画元素(变换矩阵)
├── AnimFrame (as)      — 动画帧
├── AnimAnimation (os)  — 动画(名称+帧率+帧列表)
├── AnimBank (jn)       — 动画 Bank
├── AnimFile (ls)       — 动画文件
├── BuildVert (_e)      — 构建顶点(x,y,z,u,v,w)
├── BuildAtlasRef (br)  — 构建图集引用
├── BuildFrame (Ke)     — 构建帧(pivot+顶点+canvas)
├── BuildSymbol (Cn)    — 构建符号(名称+帧列表)
├── BuildFile (Ze)      — 构建文件
└── 渲染/播放/UI 逻辑   — SolidJS 组件、动画播放器
```

---

## 二、文件上传与分发

### 入口函数 `Vi(fileEvent)`

用户通过拖拽或点击 "Open" 按钮上传文件，接受格式：`.zip`, `.json`, `.bin`, `.dyn`

```javascript
function Vi(event) {
  let files = null;
  if (event instanceof FileList) files = event.files;
  else if (event instanceof Event) files = event.target?.files;
  
  for (const file of files) {
    const [name, ext] = file.name.split(".");
    switch (ext) {
      case "zip": Cf(file, name); break;    // ZIP 包处理
      case "dyn": wf(file, name); break;    // 加密动态包
      case "bin": file.arrayBuffer().then(buf => Hs(buf)); break;  // 二进制
      case "json": file.text().then(text => js(text)); break;      // JSON
    }
  }
}
```

### 单文件路由 `Hs(buffer)` / `js(text)`

```javascript
// 二进制文件：读前4字节魔数判断类型
function Hs(buffer, texData, pngData) {
  const reader = new BinaryDataReader(buffer);
  const magic = reader.readString(4);
  if (magic === "ANIM") wc(reader).then(anim => la(anim));      // 动画
  else if (magic === "BILD") Cc(reader).then(build => oa(build, texData, pngData)); // 构建
  else alert("Unknown file");
}

// JSON文件：读 type 字段判断类型
function js(text, texData, pngData) {
  const obj = JSON.parse(text);
  if (obj.type === "Anim") { const a = new AnimFile(); a.parseJson(obj); la(a); }
  else if (obj.type === "Build") { const b = new BuildFile(); b.parseJson(obj); oa(b, texData, pngData); }
  else if (obj.type === "symbolMap") handleSymbolMap(obj);
}
```

---

## 三、DST 动画核心文件格式

DST 动画由 **3 种文件** 协同工作：

| 文件                       | 二进制魔数 | JSON type | 作用                                      |
| -------------------------- | ---------- | --------- | ----------------------------------------- |
| `anim.bin` / `anim.json`   | `ANIM`     | `"Anim"`  | 动画时序数据：哪个符号在哪帧用什么变换    |
| `build.bin` / `build.json` | `BILD`     | `"Build"` | 构建数据：符号的顶点/UV，定义贴图裁剪方式 |
| `atlas-0.tex`              | `KTEX`     | —         | 纹理图集：DXT 压缩的精灵图集              |

### 数据关系图

```
AnimFile
 └── Bank (按名称分组)
      └── Animation (动画名 + 帧率 + 方向后缀)
           └── Frame (x, y, width, height — 碰撞框)
                └── Element (symbol名, frameNum, layer名, 变换矩阵[a,b,c,d,tx,ty], zIndex)
                         │
                         │ 引用 ↓
                         │
BuildFile
 └── Symbol (按名称分组)
      └── Frame (frameNum, duration, pivot[x,y], width, height)
           ├── Vert[] (x, y, z, u, v, w) — 顶点数据，定义如何从atlas裁剪
           └── canvas — 裁剪拼装后的精灵图(Canvas对象)
                         │
                         │ 裁剪自 ↓
                         │
Atlas (Ktex)
 └── Mipmap[]
      └── blockData — DXT压缩纹理数据
```

---

## 四、anim.bin 二进制解析（`wc` 函数）

### 4.1 文件头结构

```
Offset  Size   Field          类型
0       4      Magic          char[4] = "ANIM"
4       4      Version        int32
8       4      numElements    uint32
12      4      numFrames      uint32
16      4      numEvents      uint32
20      4      numAnims       uint32
```

> **解析策略**：先跳到 `cursor=20` 预扫描 element 数据计算偏移量，再回到 `cursor=24` 正式解析。

### 4.2 预扫描阶段

预扫描的目的是跳过所有 element 数据，定位到后面的字符串哈希表：

```
cursor = 20
numAnims = readUint32()

for (i = 0; i < numAnims; i++) {
  nameLen = readUint32()                                    // 动画名长度
  frameCount = readUint32(cursor + 9 + nameLen)             // 跳跃读取帧数
  for (j = 0; j < frameCount; j++) {
    elementCount = readUint32(cursor + 16)                  // 跳跃读取元素数
    skipSize = readUint32(cursor + elementCount * 4)        // 跳跃读取跳过字节数
    cursor += 40 * elementCount                             // 每个element占40字节
  }
}
```

### 4.3 字符串哈希表

DST 使用哈希值代替字符串索引，解析时先读取映射表还原字符串：

```
numHashEntries = readUint32()
for (i = 0; i < numHashEntries; i++) {
  hash = readUint32()            // 32位哈希值
  strLen = readInt32()           // 字符串长度
  string = readString(strLen)    // ASCII字符串
  hashMap.set(hash, string)
}
```

**哈希函数 `Ne(name)`**：

```javascript
function Ne(name) {
  let hash = 0n;
  for (const char of name) {
    hash = (BigInt(char.toLowerCase().charCodeAt(0)) 
           + (hash << 6n) + (hash << 16n) - hash) & 0xffffffffn;
  }
  return Number(hash);
}
```

### 4.4 方向后缀映射

DST 动画名根据方向标志位添加后缀：

```javascript
// 方向标志位定义
const RIGHT  = 1;    // 0x01
const UP     = 2;    // 0x02
const LEFT   = 4;    // 0x04
const DOWN   = 8;    // 0x08
const UPRIGHT  = 16;  // 0x10
const UPLEFT   = 32;  // 0x20
const DOWNLEFT = 64;  // 0x40
const DOWNRIGHT= 128; // 0x80

// 方向 → 后缀映射
const directionSuffixMap = new Map([
  [UP,              "_up"],        // 2
  [DOWN,            "_down"],      // 8
  [LEFT|RIGHT,      "_side"],      // 5
  [LEFT,            "_left"],      // 4
  [RIGHT,           "_right"],     // 1
  [UPLEFT|UPRIGHT,  "_upside"],    // 48
  [DOWNLEFT|DOWNRIGHT, "_downside"], // 192
  [UPLEFT,          "_upleft"],    // 32
  [UPRIGHT,         "_upright"],   // 16
  [DOWNLEFT,        "_downleft"],  // 128
  [DOWNRIGHT,       "_downright"], // 64
  [UPLEFT|UPRIGHT|DOWNLEFT|DOWNRIGHT, "_45s"], // 240
  [UP|DOWN|LEFT|RIGHT, "_90s"],    // 15
]);
```

### 4.5 正式解析

```
cursor = 24   // 跳过文件头(20) + numAnims(4)

for (i = 0; i < numAnims; i++) {
  nameLen = readInt32()
  animName = readString(nameLen)
  directionFlag = readByte()
  bankHash = readUint32()
  bankName = hashMap.get(bankHash) || String(bankHash)
  frameRate = readFloat32()
  frameCount = readInt32()
  
  animName += directionSuffixMap.get(directionFlag) || ""
  
  animation = new AnimAnimation(animName, frameRate)
  animFile.addAnimation(bankName, animation)
  
  for (j = 0; j < frameCount; j++) {
    // 帧数据
    x = readFloat32()
    y = readFloat32()
    width = readFloat32()
    height = readFloat32()
    frame = new AnimFrame(j, x, y, width, height)
    
    // 事件
    numEvents = readUint32()
    for (k = 0; k < numEvents; k++) {
      eventHash = readUint32()
      eventName = hashMap.get(eventHash) || String(eventHash)
      frame.events.push(eventName)
    }
    
    // 元素
    numElements = readUint32()   // 从当前cursor位置读取
    for (k = 0; k < numElements; k++) {
      symbolHash = readUint32()
      symbolName = hashMap.get(symbolHash) || String(symbolHash)
      frameNum = readUint32()
      layerHash = readUint32()
      layerName = hashMap.get(layerHash) || String(layerHash)
      
      // 2×3 仿射变换矩阵
      a  = readFloat32()   // scaleX / cos(rotation)
      b  = readFloat32()   // skewY / sin(rotation)
      c  = readFloat32()   // skewX / -sin(rotation)
      d  = readFloat32()   // scaleY / cos(rotation)
      tx = readFloat32()   // translateX
      ty = readFloat32()   // translateY
      
      zIndex = readFloat32()  // 图层排序值
      
      element = new AnimElement(zIndex, symbolName, frameNum, layerName, a,b,c,d,tx,ty)
      frame.elements.push(element)
    }
    
    frame.elements.sort((a, b) => a.zIndex - b.zIndex)
    animation.frames.push(frame)
  }
}
```

### 4.6 数据类定义

```
AnimElement (rs)
├── zIndex     — 图层深度
├── symbol     — 引用的 Build Symbol 名称
├── frameNum   — 引用的 Build Frame 编号
├── layerName  — 图层名称
├── a, b, c, d — 2×2 变换矩阵
├── tx, ty     — 平移向量
└── 方法: rotation(), scale(), translate(), skew(), getApplyData()

AnimFrame (as)
├── idx        — 帧索引
├── x, y       — 帧位置
├── width, height — 帧尺寸(碰撞框)
├── elements[] — 元素列表
└── events[]   — 事件列表

AnimAnimation (os)
├── name       — 动画名称(含方向后缀)
├── frameRate  — 帧率
├── frames[]   — 帧列表
└── 方法: sort(), rotation(), scale(), translate(), skew()

AnimBank (jn)
├── name        — Bank名称
├── animations[] — 动画列表
└── 方法: sort(), rotation(), scale(), translate(), skew()

AnimFile (ls)
├── version = 4
├── type = "Anim"
├── banks[]    — Bank列表
└── 方法: addAnimation(), getBank(), calculateCollisionBox(), parseJson(), jsonStringify()
```

---

## 五、build.bin 二进制解析（`Cc` 函数）

### 5.1 文件头结构

```
Offset  Size   Field          类型
0       4      Magic          char[4] = "BILD"
4       4      Version        int32
8       4      numSymbols     uint32
12      4      totalFrames    uint32
16      4      nameLen        int32
20+     nameLen  name         string
```

### 5.2 解析流程

```
cursor = 8
numSymbols = readUint32()
totalFrames = readUint32()      // 跳过，但保留偏移
nameLen = readInt32(16)         // 从偏移16读取
name = readString(nameLen)

// Atlas 引用列表
numAtlases = readUint32()
for (i = 0; i < numAtlases; i++) {
  atlasNameLen = readUint32()
  atlasName = readString(atlasNameLen)
  atlases.push(new BuildAtlasRef(atlasName))
}

// === 第一遍：跳过 Symbol 数据，定位到顶点数组 ===
savedCursor = cursor
for (i = 0; i < numSymbols; i++) {
  frameCount = readUint32(cursor + 4)   // 跳跃读取
  cursor += frameCount * 4 * 8          // 每帧占32字节(4×uint32 + 4×float32)
}

// 读取顶点数组
numVerts = readUint32()
verts = []
for (i = 0; i < numVerts; i++) {
  x = readFloat32()
  y = readFloat32()
  z = readFloat32()
  u = readFloat32()    // UV 坐标 U
  v = readFloat32()    // UV 坐标 V
  w = readFloat32()    // Atlas 索引
  verts.push(new BuildVert(x, y, z, u, v, w))
}

// 读取字符串哈希表
numHashEntries = readUint32()
for (i = 0; i < numHashEntries; i++) {
  hash = readUint32()
  strLen = readInt32()
  string = readString(strLen)
  hashMap.set(hash, string)
}

// === 第二遍：正式读取 Symbol 数据 ===
cursor = savedCursor
for (i = 0; i < numSymbols; i++) {
  symbolHash = readUint32()
  frameCount = readUint32()
  symbolName = hashMap.get(symbolHash) || String(symbolHash)
  symbol = new BuildSymbol(symbolName)
  
  for (j = 0; j < frameCount; j++) {
    frameNum = readUint32()
    duration = readUint32()
    x = readFloat32()       // Pivot X
    y = readFloat32()       // Pivot Y
    width = readFloat32()
    height = readFloat32()
    vertStartIdx = readUint32()
    vertCount = readUint32()
    
    frame = new BuildFrame(frameNum, duration, x, y, width, height)
    frame.verts = verts.slice(vertStartIdx, vertStartIdx + vertCount)
    symbol.frames.push(frame)
  }
  symbol.sort()
  symbols.push(symbol)
}
```

### 5.3 数据类定义

```
BuildVert (_e)
├── x, y, z  — 空间坐标
├── u, v     — UV纹理坐标 (0.0~1.0)
└── w        — Atlas 索引 (整数，指向哪个图集)

BuildAtlasRef (br)
├── name     — 图集名称(如 "atlas-0.tex")
└── ktex     — Ktex 对象(解析后赋值)

BuildFrame (Ke)
├── frameNum — 帧编号
├── duration — 持续时间
├── x, y     — Pivot 点(锚点)
├── width, height — 帧尺寸
├── verts[]  — 顶点数组(每6个一组，构成三角形带)
├── canvas   — 裁剪拼装后的精灵图(Canvas)
└── 方法: getRelativePivot(), setAbsolutePivot(), updateWH()

BuildSymbol (Cn)
├── name     — 符号名称
├── frames[] — 帧列表
└── 方法: sort(), getSubRows(), getFrame(), getFrameByName()

BuildFile (Ze)
├── version = 6
├── type = "Build"
├── name     — 构建名称
├── symbols[] — 符号列表
├── atlases[] — 图集引用列表
├── scale = 1
└── 方法: splitAtlas(), hasAtlas(), getSymbol(), parseJson(), jsonStringify()
```

---

## 六、KTEX 纹理解析（`Navigation-QA1c6I0M.js`）

### 6.1 文件结构

```
Offset  Size   Field
0       4      Magic = "KTEX"
4       4      specificationData (uint32) — 编码了 platform/pixelFormat/textureType/mipmapCount/flags/fill
8+      ...    Mipmap 头部列表
...     ...    Mipmap 数据块列表
末尾    0/1    preMultiplyAlpha 标志(可选)
```

### 6.2 Specification 解码

KTEX 头部的 4 字节 specificationData 是一个位域，根据版本(PreCave/PostCave)有不同的位偏移：

| 字段        | PreCave 偏移 | PreCave 位数 | PostCave 偏移 | PostCave 位数 |
| ----------- | ------------ | ------------ | ------------- | ------------- |
| Platform    | 0            | 3            | 0             | 4             |
| PixelFormat | 3            | 3            | 4             | 5             |
| TextureType | 6            | 3            | 9             | 4             |
| MipmapCount | 9            | 4            | 13            | 5             |
| Flags       | 14           | 1            | 18            | 2             |
| Fill        | 15           | 18           | 20            | 12            |

**版本判断**：如果 `(specData >> 14) & 0x3FFFF === 0x3FFFF`，则为 PreCave 规格。

### 6.3 枚举值

```
Platform:  Default=0, PC=12, PS3=10, Xbox360=11
PixelFormat: DXT1=0, DXT3=1, DXT5=2, RGBA=4, RGB=5, UNKNOWN=7
TextureType: oneD=0, twoD=1, threeD=2, cubeMapped=3
```

### 6.4 Mipmap 读取

```
for (i = 0; i < header.mipmapCount; i++) {
  width = readUint16()
  height = readUint16()
  pitch = readUint16()       // 行字节数(跳过不使用)
  dataSize = readUint32()
  mipmaps.push(new KtexMipmap(width, height, dataSize))
}

for (mipmap of mipmaps) {
  mipmap.blockData = readBytes(mipmap.dataSize)  // DXT压缩数据
}

// 可选的末尾字节
if (remainingBytes === 1) {
  preMultiplyAlpha = !!readByte()
}
```

### 6.5 KTEX → Canvas 图像

`Ktex.toImage()` 方法将 DXT 压缩数据解码为 Canvas：

1. 取最大 mipmap（第一个）
2. 根据 PixelFormat 选择解码器（DXT1/DXT3/DXT5/RGBA/RGB）
3. DXT 块解码为 RGBA 像素数据
4. 创建 Canvas，写入 ImageData
5. 如果 `preMultiplyAlpha`，在解码时预乘 Alpha

`Ktex.fromImage(image)` 方法反向编码：Canvas → DXT 压缩 → KTEX 格式

---

## 七、Build + Atlas 拼装（`splitAtlas` 方法）

这是将 build 的顶点数据与 atlas 纹理拼装成每个 Symbol Frame 独立 Canvas 图像的关键步骤。

### 7.1 流程

```
1. 将 atlas 的 .tex 文件解码为 Image（Ktex.toImage → DXT解压 → Canvas）
2. 对每个 Symbol 的每个 Frame:
   a. 遍历其 verts 数组（每6个顶点一组，构成三角形带）
   b. 计算 UV 边界: minU, maxU, minV, maxV
   c. 计算 XY 边界: minX, maxX, minY, maxY
   d. 从 atlas 图像中裁剪出对应区域（UV坐标 → 像素坐标）
   e. 用 transform() 函数对裁剪出的图像做仿射变换（根据顶点XY坐标）
   f. 将变换后的图像贴到以 pivot(x,y) 为中心的正确位置
3. 最终每个 BuildFrame 拥有一个 canvas 属性，即该帧的精灵图
```

### 7.2 关键代码逻辑

```javascript
async splitAtlas(atlasKtexMap) {
  // 1. 解码所有 atlas 为 Image
  const atlasImages = atlases.map(a => a.ktex.toImage());
  
  for (symbol of this.symbols) {
    for (frame of symbol.frames) {
      const verts = frame.verts;
      if (!verts || !verts.length) continue;
      
      const pivotX = frame.x - Math.floor(frame.width / 2);
      const pivotY = frame.y - Math.floor(frame.height / 2);
      
      // 2. 计算 UV 和 XY 边界
      let minU=Infinity, maxU=-Infinity, minV=Infinity, maxV=-Infinity;
      let minX=Infinity, maxX=-Infinity, minY=Infinity, maxY=-Infinity;
      for (let i = 0; i < verts.length; i += 6) {
        minU = Math.min(minU, verts[i].u);
        maxU = Math.max(maxU, verts[i+1].u);
        maxV = Math.max(maxV, verts[i+2].v);
        minV = Math.min(minV, verts[i+2].v);  // 注意: V坐标翻转
        minX = Math.min(minX, verts[i].x);
        maxX = Math.max(maxX, verts[i+1].x);
        maxY = Math.max(maxY, verts[i+2].y);
        minY = Math.min(minY, verts[i+3].y);
      }
      
      // 3. UV → 像素坐标，从 atlas 裁剪
      const atlasImg = atlasImages[verts[0].w];
      const srcX = Math.round(minU * atlasImg.width);
      const srcY = Math.round((1 - maxV) * atlasImg.height);  // V坐标翻转
      const srcW = Math.round((maxU - minU) * atlasImg.width);
      const srcH = Math.round((maxV - minV) * atlasImg.height);
      
      // 4. 裁剪 + 仿射变换
      let spriteCanvas = crop(atlasImg, srcX, srcY, srcW, srcH);
      
      // 5. 如果变换后尺寸与预期不匹配，进行缩放
      const expectedW = Math.round(maxX - minX);
      const expectedH = Math.round(maxY - minY);
      if (spriteCanvas.width !== expectedW || spriteCanvas.height !== expectedH) {
        spriteCanvas = resize(spriteCanvas, expectedW, expectedH);
      }
      
      // 6. 贴到以 pivot 为中心的画布上
      const destX = Math.round(minX - pivotX);
      const destY = Math.round(minY - pivotY);
      const finalCanvas = newCanvas(frame.width, frame.height);
      paste(finalCanvas, spriteCanvas, destX, destY);
      
      frame.canvas = finalCanvas;
    }
  }
}
```

---

## 八、动画渲染流程

### 8.1 渲染管线

```
1. 从 Anim 数据获取当前帧的所有 Elements
2. 对每个 Element (按 zIndex 排序):
   a. 通过 element.symbol 在 Build 中查找 Symbol
   b. 获取该 Symbol 在 element.frameNum 处的 BuildFrame
   c. BuildFrame.canvas 即为该帧的精灵图
   d. 用 Element 的变换矩阵应用 CSS transform:
      matrix(a*scale, b*scale, c*scale, d*scale, 
             tx*scale + pivotX + offsetX, 
             ty*scale + pivotY + offsetY)
   e. 将精灵图绘制到对应的 DOM 元素
3. 多个 Element 叠加形成完整动画帧
```

### 8.2 Symbol Frame 查找 `mr(buildList, symbolName, frameNum)`

```javascript
function mr(buildList, symbolName, frameNum) {
  const resolvedName = resolveAlias(symbolName) || symbolName;
  for (const build of buildList) {
    if (!build.use) continue;
    const [symbol, symbolIdx] = build.data.getSymbol(resolvedName);
    if (!symbol) continue;
    const [frame, frameIdx] = symbol.getFrame(frameNum);
    if (!frame) return;
    return frame;  // 返回 BuildFrame (含 canvas)
  }
}
```

### 8.3 帧边界计算 `Ci(animFrame)`

用于计算动画帧的整体边界框，便于居中显示：

```javascript
async function Ci(animFrame) {
  let top=Infinity, left=Infinity, bottom=-Infinity, right=-Infinity;
  
  for (element of animFrame.elements) {  // 从后往前遍历
    const buildFrame = mr(buildList, element.symbol, element.frameNum);
    if (!buildFrame?.canvas) continue;
    
    // 对精灵图应用变换矩阵
    const transformed = transform(buildFrame.canvas, element.a, element.b, element.c, element.d);
    
    // 计算变换后的边界
    const elemLeft = element.tx + buildFrame.x * element.a + buildFrame.y * element.c;
    const elemTop  = element.ty + buildFrame.x * element.b + buildFrame.y * element.d;
    
    top    = Math.min(top,    elemTop - transformed.height/2);
    bottom = Math.max(bottom, elemTop + transformed.height/2);
    left   = Math.min(left,   elemLeft - transformed.width/2);
    right  = Math.max(right,  elemLeft + transformed.width/2);
  }
  
  return { frameLeft: left, frameTop: top, frameRight: right, frameBottom: bottom };
}
```

---

## 九、.dyn 加密文件处理

### 9.1 XOR 流密码

`.dyn` 文件是 XOR 加密的 ZIP，密钥为 8 字节递增序列：

```javascript
// 密钥生成
const xr = new Uint8Array(16);
for (let i = 0; i < 16; i++) xr[i] = 141 + i;  // [141, 142, ..., 156]

// 置换表
const Ac = [5, 3, 6, 7, 4, 2, 0, 1];

// 加解密核心 (8字节块)
function xorCipher(block, encrypt) {
  if (block.length > 16) {
    const result = new Uint8Array(16);
    for (let i = 0; i < 16; i++) {
      const j = Ac[i];
      result[encrypt ? j : i] = block[encrypt ? i : j] ^ xr[i];
    }
    return result;
  }
  return block;
}
```

### 9.2 解密流程 `vr(buffer, isEncrypt)`

```javascript
async function vr(buffer, isEncrypt) {
  const reader = new BinaryDataReader(buffer);
  const writer = new BinaryDataWriter();
  
  // 读取前32字节(2个16字节块)
  let firstBlock = reader.readBytes(32);
  
  // 如果不是加密的(前2字节是"PK" = ZIP签名)，直接返回
  if (!isEncrypt && asciiDecode(firstBlock.slice(0,2)) === "PK") {
    writer.writeBytes(firstBlock);
    writer.writeBytes(reader.readBytes());
    return writer.getBuffer();
  }
  
  // XOR 解密第一块
  writer.writeBytes(xorCipher(firstBlock, isEncrypt));
  
  // 滑动窗口解密后续块
  while (true) {
    // 读取下一块，用前一块的密文作为密钥的一部分
    // ... (滑动窗口 XOR 逻辑)
  }
}
```

### 9.3 .dyn 文件处理流程 `wf(file, name)`

```
1. 读取文件为 ArrayBuffer
2. vr(buffer) 解密 → 得到 ZIP 数据
3. JSZip.loadAsync() 解压
4. 遍历 ZIP 中的 .tex 文件 → Ktex.readKtex() 解码
5. 构建 { buildName, atlases } 对象
6. 传递给 vf() 进行后续处理
```

---

## 十、ZIP 包处理

### 10.1 ZIP 包类型判断 `Cf(file, name)`

```
Cf(zipFile, name):
  JSZip.loadAsync(zipFile) → zip
  
  if zip contains "psd.json":
    → kf(zip)              // PSD 导出格式
  
  else if zip contains "anim.bin" OR "anim.json" OR "build.bin" OR "build.json":
    → _f(zip)              // 标准 DST 动画包
  
  else:
    // 尝试作为 Spine 动画处理
    查找含 "skeleton" 的 JSON 文件
    → pf(spineData, zip)   // Spine → DST 转换
```

### 10.2 标准 DST ZIP 处理 `_f(zip)`

```javascript
async function _f(zip) {
  // 1. 解析动画数据
  zip.file("anim.bin")?.async("arraybuffer").then(buf => Hs(buf));
  zip.file("anim.json")?.async("text").then(text => js(text));
  
  // 2. 解析纹理图集
  const texMap = {};
  const loadPromises = [];
  for (const path in zip.files) {
    if (path.endsWith(".tex")) {
      texMap[path] = new KtexWrapper(path);
      loadPromises.push(
        zip.files[path].async("arraybuffer").then(buf => texMap[path].readKtex(buf))
      );
    }
  }
  
  // 3. 解析 PNG 替换图
  const pngList = [];
  for (const path in zip.files) {
    let [dir, filename] = path.split("/");
    if (path.endsWith(".png") && filename) {
      filename = filename.split(".")[0];  // 去掉扩展名
      loadPromises.push(
        zip.files[path].async("blob").then(async blob => {
          const url = URL.createObjectURL(blob);
          const img = await loadImage(url);
          const canvas = newCanvas(img);
          pngList.push({ symbolName: dir, frameName: filename, canvas });
        })
      );
    }
  }
  
  // 4. 解析构建数据(传入 tex 和 png)
  zip.file("build.bin")?.async("arraybuffer").then(buf => Hs(buf, texMap, pngList));
  zip.file("build.json")?.async("text").then(text => js(text, texMap, pngList));
}
```

### 10.3 数据设置函数

```javascript
// 设置 Build 数据
async function oa(build, texMap, pngList) {
  // 1. 如果有纹理图集，执行 splitAtlas 裁剪拼装
  if (texMap) await build.splitAtlas(texMap);
  
  // 2. 如果没有图集，尝试从 PNG 替换图生成
  if (!build.hasAtlas()) generateAtlasFromPNG(build);
  
  // 3. 应用 PNG 替换图（覆盖对应 frame 的 canvas）
  if (pngList) {
    for (const { symbolName, frameName, canvas } of pngList) {
      const frame = build.getSymbol(symbolName)?.[0].getFrameByName(frameName)?.[0];
      if (frame && !frame.canvas) {
        frame.canvas = canvas;
        frame.updateWH(canvas.width, canvas.height);
      }
    }
  }
  
  // 4. 添加到全局 Build 列表
  addBuild(build);
}

// 设置 Anim 数据
function la(anim) {
  // 将所有 Bank 添加到全局 Anim 列表
  addAnimBanks(anim.banks);
}
```

---

## 十一、完整数据流图

```
用户上传文件
    │
    ├─ .bin ─→ Hs(buffer)
    │           ├─ 魔数 "ANIM" → wc(reader) → AnimFile → la() → 全局Anim列表
    │           └─ 魔数 "BILD" → Cc(reader) → BuildFile → oa() → splitAtlas() → 全局Build列表
    │
    ├─ .json ─→ js(text)
    │           ├─ type "Anim" → AnimFile.parseJson() → la()
    │           ├─ type "Build" → BuildFile.parseJson() → oa()
    │           └─ type "symbolMap" → handleSymbolMap()
    │
    ├─ .dyn ─→ vr(buffer) 解密 → ZIP → wf()
    │           └─ 解压 → .tex 文件 → Ktex.readKtex() → vf()
    │
    └─ .zip ─→ Cf(file)
               ├─ 含 psd.json → kf() (PSD格式)
               ├─ 含 anim/build → _f()
               │    ├─ anim.bin/json → wc()/parseJson() → AnimFile
               │    ├─ build.bin/json → Cc()/parseJson() → BuildFile
               │    ├─ .tex → Ktex.readKtex() → 纹理图集
               │    └─ .png → Canvas 替换图
               │         │
               │         └─ oa(build, texMap, pngList)
               │              ├─ splitAtlas(): Build.verts + Ktex图像 → 每帧Canvas
               │              └─ PNG覆盖对应frame
               │
               └─ 含 spine → pf() (Spine转换)

═══════════════════════════════════════════════════

渲染管线:
  Anim.Element(symbol, frameNum, transform)
       ×
  Build.Symbol.Frame(canvas, pivot)
       =
  CSS matrix(a,b,c,d,tx,ty) × canvas精灵图 → 动画帧
```

---

## 十二、BinaryDataReader / BinaryDataWriter

来自 `Navigation-QA1c6I0M.js`，是所有二进制解析的基础工具：

### BinaryDataReader

```javascript
class BinaryDataReader {
  constructor(buffer, cursor = 0) {
    this.buffer = buffer;          // ArrayBuffer
    this.dataView = new DataView(buffer);
    this.cursor = cursor;
  }
  
  readByte(offset = this.cursor)      → uint8,   cursor += 1
  readInt32(offset = this.cursor)     → int32,   cursor += 4  (little-endian)
  readUint32(offset = this.cursor)    → uint32,  cursor += 4  (little-endian)
  readtHex(offset = this.cursor)      → uint16,  cursor += 2  (little-endian)
  readFloat32(offset = this.cursor)   → float32, cursor += 4  (little-endian)
  readString(length, offset)          → string,  ASCII解码
  readBytes(length, offset)           → Uint8Array
}
```

### BinaryDataWriter

```javascript
class BinaryDataWriter {
  writeByte(value)        → 1字节
  writeInt32(value)       → 4字节 (little-endian)
  writeUint32(value)      → 4字节 (little-endian)
  writeHex(value)         → 2字节 (uint16, little-endian)
  writeFloat32(value)     → 4字节 (little-endian)
  writeString(value)      → UTF-8编码
  writeBytes(Uint8Array)  → 原始字节
  getBuffer()             → 合并所有写入为 Uint8Array
}
```

---

## 十三、图像处理工具函数

来自 `Navigation-QA1c6I0M.js`：

| 函数                            | 作用                    | 在流程中的位置                  |
| ------------------------------- | ----------------------- | ------------------------------- |
| `transform(canvas, a, b, c, d)` | 对图像应用 2×2 仿射变换 | splitAtlas 中变换裁剪后的精灵图 |
| `crop(canvas, x, y, w, h)`      | 从图像裁剪矩形区域      | splitAtlas 中从 atlas 裁剪      |
| `resize(canvas, w, h)`          | 缩放图像                | splitAtlas 中匹配预期尺寸       |
| `paste(dest, src, x, y)`        | 将源图像粘贴到目标画布  | splitAtlas 中贴到 pivot 位置    |
| `newCanvas(w, h)`               | 创建新 Canvas           | 贯穿全流程                      |
| `preMultiplyAlpha(canvas)`      | 预乘 Alpha 通道         | KTEX 编码/解码                  |
| `flipY(canvas)`                 | Y 轴翻转                | KTEX 编码(纹理坐标 V 翻转)      |
| `toBlob(canvas)`                | Canvas → Blob           | 导出功能                        |
| `applyColorCube(canvas, cc)`    | 应用颜色校正立方体      | 视觉效果                        |
