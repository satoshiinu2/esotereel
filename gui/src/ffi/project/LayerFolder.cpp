#include "LayerFolder.h"
#include "esotereel_gui_helper.h"
#include "ffi/StringView.h"
#include <QString>

namespace esotereel {
using RawFolder = esotereel_gui_helper::LayerFolder;

QString LayerFolder::name() const noexcept {
    return StringView::toQString(esotereel_gui_helper::layer_folder_get_name(raw_ptr));
}
} // namespace esotereel