import QtQuick

Rectangle {
    color: Theme.surface
    radius: Theme.popupRadius
    border.color: Theme.line
    BorderImage {
        anchors.fill: parent
        anchors.margins: -16
        anchors.topMargin: -12
        anchors.bottomMargin: -20
        z: -1
        source: "assets/popup-shadow.png"
        border {
            left: 24
            top: 24
            right: 24
            bottom: 24
        }
        opacity: Theme.dark ? 0.5 : 1
    }
}
