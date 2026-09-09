pragma Singleton
import QtQuick

QtObject {
    id: theme
    property string appearance: "system"
    property bool reducedMotion: false
    property bool compactGrid: true
    property bool coverTheme: true
    property bool ambientMotion: true
    property bool lyricSecondary: true
    property string previewAppearance: ""
    readonly property string effectiveAppearance: previewAppearance || appearance
    readonly property bool dark: effectiveAppearance === "dark" || (effectiveAppearance === "system" && Application.styleHints.colorScheme === Qt.Dark)
    readonly property color background: dark ? "#18191b" : "#faf9f6"
    readonly property color sidebar: dark ? "#141517" : "#f1f0ec"
    readonly property color surface: dark ? "#222326" : "#ffffff"
    readonly property color subtle: dark ? "#2b2c30" : "#eeede9"
    readonly property color text: dark ? "#edece8" : "#242522"
    readonly property color secondary: dark ? "#b0b0aa" : "#62645e"
    readonly property color muted: dark ? "#9b9c96" : "#6d6e67"
    readonly property color line: dark ? "#343538" : "#deded7"
    readonly property color accent: dark ? "#d5aa86" : "#885b38"
    readonly property color accentWash: dark ? "#342b25" : "#eee5da"
    readonly property color danger: dark ? "#efa49c" : "#a33f36"
    readonly property string fontFamily: Qt.application.font.family
    readonly property int titleSize: 26
    readonly property int headingSize: 18
    readonly property int bodySize: 14
    readonly property int labelSize: 13
    readonly property int captionSize: 12
    readonly property int noteSize: 11
    readonly property int space1: 4
    readonly property int space2: 8
    readonly property int space3: 12
    readonly property int space4: 16
    readonly property int space6: 24
    readonly property int space8: 32
    readonly property int space10: 40
    readonly property int fast: reducedMotion ? 0 : 90
    readonly property int medium: reducedMotion ? 0 : 140
    readonly property int slow: reducedMotion ? 0 : 200
    readonly property int radius: 8
    readonly property int smallRadius: 6
    readonly property int popupRadius: 12
    readonly property int rowHeight: 56
    readonly property int buttonHeight: 38
    readonly property int compactButton: 32
    function time(milliseconds) {
        const seconds = Math.max(0, Math.floor(milliseconds / 1000));
        return Math.floor(seconds / 60) + ":" + String(seconds % 60).padStart(2, "0");
    }
}
