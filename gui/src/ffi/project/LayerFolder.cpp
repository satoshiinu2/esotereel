#include "LayerFolder.h"
#include "esotereel_gui_helper.h"
#include "ffi/StringView.h"
#include <QString>

namespace esotereel {
using RawFolder = esotereel_gui_helper::LayerFolder;

QString LayerFolder::name() const noexcept {
    auto result = esotereel_gui_helper::layer_folder_get_name(raw_ptr);
    if (!result.is_ok) {
        return QString(); // Return empty string on error
    }

    return StringView::toQString(result.value.ok);
}
} // namespace esotereel