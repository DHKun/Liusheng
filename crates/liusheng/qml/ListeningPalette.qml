import QtQuick

// Constrained semantic roles: artwork supplies hue; text retains readable contrast.
QtObject {
    id: colors
    property color seed: "#6f9d99"
    property bool tinted: true
    property bool dark: Theme.dark
    readonly property color base: dark ? "#15191b" : "#f8f6f0"
    readonly property color background: tinted ? mix(base, seed, dark ? 0.13 : 0.12) : Theme.background
    readonly property color edge: tinted ? mix(base, seed, dark ? 0.21 : 0.21) : Theme.background
    readonly property color surface: mix(background, dark ? "#ffffff" : "#ffffff", dark ? 0.055 : 0.56)
    readonly property color text: dark ? "#f5f3ed" : "#202724"
    readonly property color secondary: dark ? "#c2c9c4" : "#414b45"
    readonly property color accent: readable(tinted ? seed : Theme.accent, edge, surface, 4.5)
    readonly property color line: mix(background, text, 0.16)
    function mix(first, second, amount) {
        const a = Qt.tint("transparent", first), b = Qt.tint("transparent", second);
        return Qt.rgba(a.r + (b.r - a.r) * amount, a.g + (b.g - a.g) * amount, a.b + (b.b - a.b) * amount, 1);
    }
    function luminance(c) {
        const rgb = [c.r, c.g, c.b].map(v => v <= 0.04045 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4));
        return rgb[0] * 0.2126 + rgb[1] * 0.7152 + rgb[2] * 0.0722;
    }
    function contrast(a, b) {
        const x = luminance(a), y = luminance(b);
        return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05);
    }
    function readable(source, first, second, ratio) {
        const target = dark ? "#ffffff" : "#101714";
        for (let step = 0; step <= 20; step++) {
            const candidate = mix(source, target, step / 20);
            if (contrast(candidate, first) >= ratio && contrast(candidate, second) >= ratio)
                return candidate;
        }
        return mix(source, target, 1);
    }
}
