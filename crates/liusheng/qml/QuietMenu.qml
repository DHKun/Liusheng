pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic

Menu {
    id: menu
    // In-scene popups avoid Wayland xdg_popup grabs and native QMenu windows.
    popupType: Popup.Item
    parent: Overlay.overlay
    margins: 12
    implicitWidth: 224
    padding: 6
    focus: true
    closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
    font.family: Theme.fontFamily
    font.pixelSize: Theme.labelSize
    background: QuietSurface {}
    delegate: QuietMenuItem {}
    function openAt(anchor, localX, localY) {
        if (!anchor || !parent)
            return;
        const point = anchor.mapToItem(parent, localX, localY);
        x = Math.max(margins, Math.min(point.x, parent.width - width - margins));
        const requestedHeight = Math.min(implicitHeight, parent.height - margins * 2);
        y = Math.max(margins, Math.min(point.y, parent.height - requestedHeight - margins));
        open();
    }
    function openBelow(anchor) {
        if (anchor)
            openAt(anchor, anchor.width - width, anchor.height + 6);
    }
    enter: Transition {
        NumberAnimation {
            property: "opacity"
            from: 0
            to: 1
            duration: Theme.fast
        }
    }
    exit: Transition {
        NumberAnimation {
            property: "opacity"
            to: 0
            duration: Theme.fast
        }
    }
}
