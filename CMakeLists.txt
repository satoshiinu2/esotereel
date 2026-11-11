cmake_minimum_required(VERSION 3.16)
project(EsotereelGui)
set(CMAKE_RUNTIME_OUTPUT_DIRECTORY "${CMAKE_CURRENT_BINARY_DIR}")

# clangdがプロジェクト全体を解析できるようにビルドフラグを出力
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)
set(QT_NO_PRIVATE_MODULE_WARNING ON CACHE BOOL "" FORCE)

# cmakeの設定
set(CMAKE_CXX_STANDARD 23)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)
set(CMAKE_AUTOUIC ON)
set(CMAKE_AUTORCC ON)

set(ESOTEREEL_ENABLE_GPU_PREVIEW ON)

# Allow setting Qt6 path via environment variable
if(NOT Qt6_DIR)
    if(DEFINED ENV{Qt6_DIR})
        list(APPEND CMAKE_PREFIX_PATH "$ENV{Qt6_DIR}")
    elseif(DEFINED ENV{QT_DIR})
        list(APPEND CMAKE_PREFIX_PATH "$ENV{QT_DIR}")
    endif()
endif()

# ビルドタイプに応じた設定の切り替え
if(NOT CMAKE_BUILD_TYPE OR CMAKE_BUILD_TYPE STREQUAL "Debug")
    set(IS_DEBUG TRUE)
else()
    set(IS_DEBUG FALSE)
endif()

if(IS_DEBUG)
    set(RUST_PROFILE "debug")
    set(RUST_FLAG "")
else()
    set(RUST_PROFILE "release")
    set(RUST_FLAG "--release")
endif()

# ライブラリの拡張子とか
if(WIN32)
    set(DLIB_EXT "dll")
    set(RLIB_PREFIX "")
    set(CLANGD_OS "windows")
    set(CLANGD_DEFINES "    \"-DUNICODE\",\n    \"-D_UNICODE\",")
    set(CLANGD_SYS_INC "")
elseif(APPLE)
    set(DLIB_EXT "dylib")
    set(RLIB_PREFIX "lib")
    set(CLANGD_OS "darwin")
    set(CLANGD_DEFINES "")
    set(CLANGD_SYS_INC "")
else()
    set(DLIB_EXT "so")
    set(RLIB_PREFIX "lib")
    set(CLANGD_OS "linux")
    set(CLANGD_DEFINES "")
    set(CLANGD_SYS_INC "    \"-isystem\", \"/usr/include\",")
endif()

# Qt6 components - Wayland is platform-specific
set(QT_COMPONENTS Widgets Network Gui Core LinguistTools)

find_package(Qt6 REQUIRED COMPONENTS ${QT_COMPONENTS})

# qt ads
include(FetchContent)
FetchContent_Declare(
    ads
    GIT_REPOSITORY https://github.com/githubuser0xFFFF/Qt-Advanced-Docking-System.git
    GIT_TAG        5.0.0  # Using stable version with wayland support
)
set(BUILD_EXAMPLES OFF CACHE BOOL "" FORCE)
set(ADS_VERSION "5.0.0" CACHE STRING "" FORCE)  # Match the tag
FetchContent_MakeAvailable(ads)

# ADSターゲットが存在する場合、そのターゲットに対する警告をすべて無効化する
if (TARGET qtadvanceddocking-qt6)
    if(NOT MSVC)
        # GCC/Clang用: すべての警告を抑制 (-w)
        target_compile_options(qtadvanceddocking-qt6 PRIVATE "-w")
    else()
        # MSVC用
        target_compile_options(qtadvanceddocking-qt6 PRIVATE "/W0")
    endif()
endif()

# Qt6のインクルードパスを、clangdが読めるフォーマットに一括変換
set(CLANGD_QT_PATHS "")
if(TARGET Qt6::Core)
    get_target_property(QT6_INC_DIRS Qt6::Core INTERFACE_INCLUDE_DIRECTORIES)
    foreach(dir ${QT6_INC_DIRS})
        # セミコロンやリストのバグを防ぐため、1つずつ安全に整形して蓄積
        set(CLANGD_QT_PATHS "${CLANGD_QT_PATHS}    \"-I${dir}\",\n")
    endforeach()
endif()

# ライブラリとコア
set(RUST_TARGET_DIR "${CMAKE_SOURCE_DIR}/build/target/${RUST_PROFILE}")
set(GUI_HLP_LIB "${RUST_TARGET_DIR}/${RLIB_PREFIX}esotereel_gui_helper.${DLIB_EXT}")
# MSVCはDLL本体を直接リンクできないため、リンク時は対応するインポートライブラリ(.dll.lib)を使う
if(WIN32)
    set(GUI_HLP_LINK_LIB "${GUI_HLP_LIB}.lib")
else()
    set(GUI_HLP_LINK_LIB "${GUI_HLP_LIB}")
endif()
set(CORE_BIN "${RUST_TARGET_DIR}/esotereel_core")
file(GLOB_RECURSE RUST_SRC_FILES
    "${CMAKE_SOURCE_DIR}/core/src/*"
    "${CMAKE_SOURCE_DIR}/lib/src/*"
    "${CMAKE_SOURCE_DIR}/guihlp/src/*"
    "${CMAKE_SOURCE_DIR}/Cargo.toml"
)

# rustビルドするやつ
set(RUST_BUILD_ENV "CARGO_TARGET_DIR=${CMAKE_SOURCE_DIR}/build/target")

# GUI Helper (ライブラリ) のビルドコマンド
set(GUI_HLP_OUTPUTS ${GUI_HLP_LIB})
if(WIN32)
    list(APPEND GUI_HLP_OUTPUTS ${GUI_HLP_LINK_LIB})
endif()

add_custom_command(
    OUTPUT ${GUI_HLP_OUTPUTS}
    COMMAND ${CMAKE_COMMAND} -E env ${RUST_BUILD_ENV} cargo build ${RUST_FLAG} -p esotereel_gui_helper
    WORKING_DIRECTORY ${CMAKE_SOURCE_DIR}
    DEPENDS ${RUST_SRC_FILES}
    USES_TERMINAL 
    COMMENT "Building Rust GUI Helper (${RUST_PROFILE})..."
)

# Core (サーバー) のビルドコマンド
add_custom_command(
    OUTPUT ${CORE_BIN}
    COMMAND ${CMAKE_COMMAND} -E env ${RUST_BUILD_ENV} cargo build ${RUST_FLAG} -p esotereel_core
    WORKING_DIRECTORY ${CMAKE_SOURCE_DIR}
    DEPENDS ${RUST_SRC_FILES}
    USES_TERMINAL 
    COMMENT "Building Rust Core Server (${RUST_PROFILE})..."
)

# GUI実行に必要なヘルパーライブラリのみを自動ビルド対象にする
add_custom_target(rust_gui_helper ALL DEPENDS ${GUI_HLP_OUTPUTS})

# サーバー側は必要なときだけ単体でビルドできるように分離 (ALL を外す)
add_custom_target(rust_core DEPENDS ${CORE_BIN})

# Clangのエラー回避
string(REPLACE "-mno-direct-extern-access" "" CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS}")

# 実行ファイル
file(GLOB_RECURSE SRC_FILES CONFIGURE_DEPENDS gui/src/*.cpp gui/src/*.h)
if(NOT ESOTEREEL_ENABLE_GPU_PREVIEW)
    list(REMOVE_ITEM SRC_FILES
        "${CMAKE_SOURCE_DIR}/gui/src/window/widget/preview/GpuPreviewWidget.cpp"
        "${CMAKE_SOURCE_DIR}/gui/src/window/widget/preview/GpuPreviewWidget.h"
        "${CMAKE_SOURCE_DIR}/gui/src/window/widget/preview/GpuRenderWorker.cpp"
        "${CMAKE_SOURCE_DIR}/gui/src/window/widget/preview/GpuRenderWorker.h"
    )
endif()
add_executable(esotereel_gui ${SRC_FILES})

# ライブラリリンク
target_include_directories(esotereel_gui PRIVATE "${CMAKE_SOURCE_DIR}/guihlp/include" "${CMAKE_SOURCE_DIR}/gui/src")
target_link_libraries(esotereel_gui PRIVATE 
    Qt6::Widgets 
    Qt6::Network
    qtadvanceddocking-qt6 
    ${GUI_HLP_LINK_LIB}
)

# フラグ設定
if(ESOTEREEL_ENABLE_GPU_PREVIEW)
    target_compile_definitions(esotereel_gui PRIVATE
        ESOTEREEL_ENABLE_GPU_PREVIEW
    )
endif()

# MSVC の場合のみ、標準準拠モード (/permissive-) を無効化する。
# guihlp/include/esotereel_gui_helper.h (cbindgen が Rust の FFI から生成) は、
# extern "C" 関数が FfiResult<T> テンプレートの実体化型を値で返す宣言を含む。
# これらの型は bool と POD/ポインタの union からなる trivially copyable な
# 集約体であり、この戻り値の渡し方自体が Rust 側がエクスポートする ABI で
# あるため、C++ 側の宣言を変えることはできない。
# GCC/Clang はこの宣言を受理するが、MSVC は /std:c++latest によって暗黙に
# 有効になる /permissive- モードでは C2526 (C リンケージ関数は C++ クラス型
# を返せない) として全宣言を拒否する。なお FfiResultVoid のような非テン
# プレートの結果型は受理されるため、拒否されているのは ABI そのものでは
# なくテンプレート実体化型の戻り値である。
# そのため、このヘッダーを include する唯一のターゲットである
# esotereel_gui に対してのみ、MSVC の従来モード (/permissive) を使う。
# これは警告の抑制ではなく、上記の宣言形式 (trivially copyable な構造体を
# C リンケージ関数が値で返す MSVC 拡張) が利用できる言語モードの選択であり、
# 他コンパイラの受理挙動に合わせるためのものである。
if(MSVC)
    target_compile_options(esotereel_gui PRIVATE /permissive)
endif()

# windows向けの依存関係の解決
if(WIN32)
    add_custom_command(TARGET esotereel_gui POST_BUILD  
        COMMAND "${CMAKE_COMMAND}" -E copy_if_different
            "$<TARGET_FILE:qtadvanceddocking-qt6>"
            "$<TARGET_FILE_DIR:esotereel_gui>"
        VERBATIM
        COMMENT "Deploying Qt Advanced Docking System"
    )
endif()

# デバッガー
# if(MSVC)
#     target_compile_options(esotereel_gui PRIVATE
#         "$<$<CONFIG:Debug>:/fsanitize=address>"
#     )
# elseif(CMAKE_CXX_COMPILER_ID MATCHES "GNU|Clang")
#     target_compile_options(esotereel_gui PRIVATE
#         "$<$<CONFIG:Debug>:-fsanitize=address>"
#         "$<$<CONFIG:Debug>:-fno-omit-frame-pointer>"
#     )

#     target_link_options(esotereel_gui PRIVATE
#         "$<$<CONFIG:Debug>:-fsanitize=address>"
#     )
# endif()

# translate 
qt_add_translations(esotereel_gui
    TS_FILES
        gui/lang/esotereel_ja.ts  
        gui/lang/esotereel_en.ts  
)

# .clangd ファイルの動的生成設定
set(CLANGD_REMOVE_FLAG "  Remove: [-mno-direct-extern-access]\n")
set(CLANGD_CONTENT
"If:
  OS: ${CLANGD_OS}
CompileFlags:
  CompilationDatabase: build/cmake
${CLANGD_REMOVE_FLAG}
  Add: [
    \"-Igui\",
    \"-Iguihlp/include\",
    \"-Ibuild/_deps/ads-src/src\",
${CLANGD_QT_PATHS}${CLANGD_SYS_INC}
${CLANGD_DEFINES}
    \"-std=c++23\"
  ]
Index:
  Background: Build
"
)

file(WRITE "${CMAKE_CURRENT_SOURCE_DIR}/.clangd" "${CLANGD_CONTENT}")

# windows向けの依存関係の解決
if(WIN32)
    find_program(ESOTEREEL_POWERSHELL
        NAMES pwsh powershell
        REQUIRED
    )

    # Rust DLL を先にビルドしてから GUI をビルドする
    add_dependencies(esotereel_gui rust_gui_helper)

    # 開発用の実行環境をまとめて準備する
    add_custom_target(setup_windows_dev ALL
        COMMAND "${ESOTEREEL_POWERSHELL}"
            -NoProfile
            -ExecutionPolicy Bypass
            -File "${PROJECT_SOURCE_DIR}/scripts/setup-windows-dev.ps1"
            -Configuration "$<CONFIG>"
        WORKING_DIRECTORY "${PROJECT_SOURCE_DIR}"
        VERBATIM
    )

    add_dependencies(setup_windows_dev esotereel_gui)
endif()
