-- Lua 5.2 through 5.4 syntax that a Lua 5.1-only parser rejects.
local limit <const> = 8
local handle <close> = nil

warn("selene smoke")
coroutine.close(coroutine.create(print))

for i = 1, limit do
    if i & 1 == 0 then
        goto continue
    end
    print(i // 2, i << 1, ~i, handle)
    ::continue::
end
