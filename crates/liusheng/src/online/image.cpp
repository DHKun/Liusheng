#include "storage.h"
#include <QtCore/QBuffer>
#include <QtGui/QImageReader>
#include <stdexcept>

namespace liusheng::online {
ImageData normalizeImage(const QByteArray &bytes) {
    auto fail = [](const char *text) { throw std::runtime_error(text); };
    if (bytes.isEmpty() || bytes.size() > ImageLimit) fail("封面文件超过 8 MiB 或为空");
    QBuffer buffer; buffer.setData(bytes); buffer.open(QIODevice::ReadOnly);
    QImageReader reader(&buffer);
    const auto format = reader.format().toLower();
    if (format != "jpeg" && format != "jpg" && format != "png" && format != "webp") fail("封面仅支持 JPEG、PNG、WebP");
    const auto size = reader.size();
    if (!size.isValid() || size.width() > 8192 || size.height() > 8192 || qint64(size.width()) * size.height() > 16000000) fail("封面像素尺寸超出限制");
    reader.setAutoTransform(true);
    reader.setScaledSize(size.scaled(1200, 1200, Qt::KeepAspectRatio));
    auto image = reader.read();
    if (image.isNull()) fail("封面解码失败");
    image = image.scaled(1200, 1200, Qt::KeepAspectRatio, Qt::SmoothTransformation).convertToFormat(QImage::Format_RGB32);
    const auto small = image.scaled(24, 24, Qt::IgnoreAspectRatio, Qt::SmoothTransformation);
    qint64 red = 0, green = 0, blue = 0;
    for (int y = 0; y < small.height(); ++y) for (int x = 0; x < small.width(); ++x) {
        const auto pixel = small.pixelColor(x, y);
        red += pixel.red(); green += pixel.green(); blue += pixel.blue();
    }
    const auto count = small.width() * small.height();
    ImageData result;
    result.accent = QColor(int(red / count), int(green / count), int(blue / count)).name();
    QBuffer output(&result.jpeg); output.open(QIODevice::WriteOnly);
    if (!image.save(&output, "JPG", 88)) fail("封面转换失败");
    return result;
}
}
