pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic

Popup {
    id: popup
    popupType: Popup.Item
    parent: Overlay.overlay
    margins: 12
    padding: 20
    focus: true
    closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
    background: QuietSurface {}
    function openBelow(anchor) {
        if (!anchor || !parent)
            return;
        const point = anchor.mapToItem(parent, anchor.width - width, anchor.height + 6);
        x = Math.max(margins, Math.min(point.x, parent.width - width - margins));
        y = Math.max(margins, Math.min(point.y, parent.height - height - margins));
        open();
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
