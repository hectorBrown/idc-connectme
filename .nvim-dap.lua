local dap = require("dap")
dap.adapters.lldb = {

	type = "executable",
	command = "/home/hex/.local/share/nvim/mason/bin/codelldb", -- adjust path if needed
	name = "lldb",
}
dap.configurations.rust = {
	{
		name = "Launch",
		type = "lldb",
		request = "launch",
		program = function()
			local output = vim.fn.system("cargo build")
			local exit_code = vim.v.shell_error
			if exit_code == 0 then
				return vim.fn.getcwd() .. "/target/debug/idc-connectme"
			else
				error("Build failed!")
				Snacks.notify(output)
			end
		end,
		cwd = "${workspaceFolder}",
		stopOnEntry = false,
		args = function()
			return { vim.fn.input("Arguments: ") }
		end,
	},
}
