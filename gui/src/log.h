
#include <cstddef>
namespace esotereel_gui_helper {
struct StringView;
} // namespace esotereel_gui_helper

void q_log_callback(size_t level, esotereel_gui_helper::StringView target_view,
                    esotereel_gui_helper::StringView msg_view);