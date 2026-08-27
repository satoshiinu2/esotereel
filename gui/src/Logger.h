
#include <cstddef>
namespace esotereel_gui_helper {
struct StringView;
} // namespace esotereel_gui_helper

namespace esotereel {
void qtLogCallback(size_t level, esotereel_gui_helper::StringView target_view,
                   esotereel_gui_helper::StringView msg_view);
}