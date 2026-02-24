-- maxmux-navigator.lua
-- Seamless navigation between Neovim splits and MaxMux panes.
-- Like vim-tmux-navigator, but for MaxMux.
--
-- Usage (pick one):
--
--   1. Source directly in init.lua:
--      vim.cmd("source /path/to/maxmux-navigator.lua")
--
--   2. With lazy.nvim:
--      { dir = "/path/to/extras/neovim", name = "maxmux-navigator" }
--
--   3. Require with options:
--      require("maxmux-navigator").setup({ maxmux_executable = "/path/to/maxmux" })
--
-- Diagnostics:  :MaxMuxCheck

local M = {}

M.config = {
	maxmux_executable = "bun run /code/maxmux/src/index.ts",
}

local direction_map = {
	h = { wincmd = "h", flag = "-L", desc = "left" },
	j = { wincmd = "j", flag = "-D", desc = "down" },
	k = { wincmd = "k", flag = "-U", desc = "up" },
	l = { wincmd = "l", flag = "-R", desc = "right" },
}

local function is_maxmux()
	-- $MAXMUX is set by MaxMux when spawning PTYs
	if vim.env.MAXMUX then
		return true
	end
	-- Fallback: check if the server socket exists
	local socket = vim.fn.expand("~/.maxmux/server.sock")
	return vim.fn.filereadable(socket) == 1 or vim.loop.fs_stat(socket) ~= nil
end

local function navigate(direction)
	local info = direction_map[direction]
	if not info then
		return
	end

	local cur_winnr = vim.fn.winnr()
	vim.cmd("wincmd " .. info.wincmd)
	local new_winnr = vim.fn.winnr()

	if cur_winnr ~= new_winnr then
		-- Moved to another Neovim split — done
		return
	end

	-- At the edge of Neovim splits — delegate to MaxMux
	if not is_maxmux() then
		return
	end

	-- Build command list: split executable string into parts (supports
	-- multi-word commands like "bun run /path/to/index.ts")
	local cmd = {}
	for part in M.config.maxmux_executable:gmatch("%S+") do
		table.insert(cmd, part)
	end
	table.insert(cmd, "select-pane")
	table.insert(cmd, info.flag)
	-- Use jobstart (async) to avoid blocking Neovim if the command hangs
	vim.fn.jobstart(cmd, { detach = true })
end

local function setup_keymaps()
	local opts = { noremap = true, silent = true }

	for dir, info in pairs(direction_map) do
		local key = "<C-" .. dir .. ">"
		local desc = "Navigate " .. info.desc .. " (Neovim/MaxMux)"

		vim.keymap.set({ "n", "t" }, key, function()
			navigate(dir)
		end, vim.tbl_extend("force", opts, { desc = desc }))
	end

	vim.api.nvim_create_autocmd("FileType", {
		pattern = "oil",
		callback = function()
			for dir, info in pairs(direction_map) do
				local key = "<C-" .. dir .. ">"
				local desc = "Navigate " .. info.desc .. " (Neovim/MaxMux)"

				vim.keymap.set({ "n", "t" }, key, function()
					navigate(dir)
				end, { desc = desc, buffer = true })
			end
		end,
	})
end

local function setup_commands()
	vim.api.nvim_create_user_command("MaxMuxCheck", function()
		local exe = M.config.maxmux_executable
		local exe_first = exe:match("%S+") or exe
		local exe_ok = vim.fn.executable(exe_first) == 1
		local in_maxmux = is_maxmux()
		local env = vim.env.MAXMUX or "(not set)"
		local socket = vim.fn.expand("~/.maxmux/server.sock")
		local socket_ok = vim.fn.filereadable(socket) == 1 or vim.loop.fs_stat(socket) ~= nil

		local lines = {
			"MaxMux Navigator Check",
			string.rep("─", 40),
			"Executable:   " .. exe .. (exe_ok and "  OK" or "  NOT FOUND"),
			"$MAXMUX:      " .. env,
			"Socket:       " .. socket .. (socket_ok and "  OK" or "  not found"),
			"Inside MaxMux: " .. (in_maxmux and "yes" or "no"),
			"",
			"Keymaps (normal mode):",
		}

		for _, dir in ipairs({ "h", "j", "k", "l" }) do
			local maps = vim.api.nvim_get_keymap("n")
			local found = false
			for _, m in ipairs(maps) do
				if m.lhs == "<C-" .. string.upper(dir) .. ">" or m.lhs == "<C-" .. dir .. ">" then
					found = true
					table.insert(lines, "  <C-" .. dir .. ">  " .. (m.desc or "mapped") .. "  OK")
					break
				end
			end
			if not found then
				table.insert(lines, "  <C-" .. dir .. ">  NOT MAPPED")
			end
		end

		print(table.concat(lines, "\n"))
	end, {})
end

function M.setup(opts)
	opts = opts or {}
	if opts.maxmux_executable then
		M.config.maxmux_executable = opts.maxmux_executable
	end
	-- vim.g override takes priority
	if vim.g.maxmux_executable then
		M.config.maxmux_executable = vim.g.maxmux_executable
	end

	setup_keymaps()
	setup_commands()
end

-- Auto-setup when sourced directly (vim.cmd("source ..."))
-- When using require(), call setup() explicitly for custom options
if not ... or ... == true then
	M.setup()
else
	-- Loaded via require — still auto-setup with defaults
	-- User can call setup() again with options to override
	M.setup()
end

return M
