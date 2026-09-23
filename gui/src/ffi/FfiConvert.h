#pragma once

#include <optional>

#include <QVector>

#include "Array.h"
#include "Option.h"
#include "Result.h"

namespace esotereel {

namespace detail {
// converterが例外を投げてもFfiArrayを解放するための最小限のスコープガード。
// 公開APIではなくconvertArrayResult内部だけの実装詳細。
template <typename T> struct ArrayFreeGuard {
    Array::FfiArray<T> &raw;
    ~ArrayFreeGuard() {
        Array::free(raw);
    }
};
} // namespace detail

// FfiResult<FfiArray<Raw>> を受け取り、各要素をconverter(Raw->Converted)で変換して
// Result<QVector<Converted>> にする。
template <typename Raw, typename Converter>
auto convertArrayResult(FfiResult<Array::FfiArray<Raw>> raw, Converter converter)
    -> Result<QVector<decltype(converter(std::declval<const Raw &>()))>> {
    using Converted = decltype(converter(std::declval<const Raw &>()));
    using Out = Result<QVector<Converted>>;

    if (!raw.is_ok) {
        return Out::err(OwnedString::intoStdString(raw.value.err));
    }

    Array::FfiArray<Raw> array = raw.value.ok;
    detail::ArrayFreeGuard<Raw> guard{array};

    QVector<Converted> out;
    const size_t n = Array::size(array);
    out.reserve(static_cast<int>(n));
    const Raw *data = Array::data(array);
    for (size_t i = 0; i < n; ++i) {
        out.append(converter(data[i]));
    }

    return Out::ok(std::move(out));
}

// FfiResult<FfiOption<Raw>> を受け取り、converter(Raw->Converted)で変換して
// Result<std::optional<Converted>> にする。
template <typename Raw, typename Converter>
auto convertOptionResult(FfiResult<Option::FfiOption<Raw>> raw, Converter converter)
    -> Result<std::optional<decltype(converter(std::declval<const Raw &>()))>> {
    using Converted = decltype(converter(std::declval<const Raw &>()));
    using Out = Result<std::optional<Converted>>;

    if (!raw.is_ok) {
        return Out::err(OwnedString::intoStdString(raw.value.err));
    }

    if (Option::isNone(raw.value.ok)) {
        return Out::ok(std::nullopt);
    }

    return Out::ok(converter(Option::unwrap(raw.value.ok)));
}

// FfiResult<Raw> (単一値、配列でもOptionでもない場合) を
// converter(Raw->Converted)で変換して Result<Converted> にする。
template <typename Raw, typename Converter>
auto convertResult(FfiResult<Raw> raw, Converter converter)
    -> Result<decltype(converter(std::declval<const Raw &>()))> {
    using Converted = decltype(converter(std::declval<const Raw &>()));
    using Out = Result<Converted>;

    if (!raw.is_ok) {
        return Out::err(OwnedString::intoStdString(raw.value.err));
    }

    return Out::ok(converter(raw.value.ok));
}

} // namespace esotereel
