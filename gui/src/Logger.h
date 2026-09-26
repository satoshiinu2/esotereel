
#include <cstddef>
namespace esotereel_gui_helper {
struct FfiStringView;
} // namespace esotereel_gui_helper

namespace esotereel {
void qtLogCallback(size_t level, esotereel_gui_helper::FfiStringView target_view,
                   esotereel_gui_helper::FfiStringView msg_view);
}