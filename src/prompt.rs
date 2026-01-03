use std::env;
use chrono::{Local, DateTime};

pub const CHAT_PROMPT: &'static str = "```markdown
# TODO LIST RECOMMENDED
When starting a new task, it is recommended to create a todo list.




1. Include the task_progress parameter in your next tool call

2. Create a comprehensive checklist of all steps needed

3. Use markdown format: - [ ] for incomplete, - [x] for complete



**Benefits of creating a todo list now:**

	- Clear roadmap for implementation

	- Progress tracking throughout the task

	- Nothing gets forgotten or missed

	- Users can see, monitor, and edit the plan



**Example structure:**
```

- [ ] Analyze requirements

- [ ] Set up necessary files

- [ ] Implement main functionality

- [ ] Handle edge cases

- [ ] Test the implementation

- [ ] Verify results
";

/// 构建增强的系统prompt，包含当前时间和工作目录信息
pub fn build_enhanced_prompt(base_prompt: &str) -> String {
    // 获取当前时间
    let now: DateTime<Local> = Local::now();
    let formatted_time = now.format("%Y-%m-%d %H:%M:%S").to_string();
    
    // 获取当前工作目录
    let current_dir = env::current_dir()
        .map(|path| path.to_string_lossy().to_string())
        .unwrap_or_else(|_| "无法获取当前目录".to_string());
    
    // 获取系统信息
    let os_info = get_system_info();
    
    // 构建增强的prompt
    format!(
        "{}\n当前时间: {}\n当前工作目录: {}\n\n{}",
        os_info, formatted_time, current_dir, base_prompt
    )
}

/// 获取系统信息
fn get_system_info() -> String {
    let os_name = env::consts::OS;
    let arch = env::consts::ARCH;
    let family = env::consts::FAMILY;

    format!(
        "操作系统: {}\n架构: {}\n系统家族: {}",
        os_name, arch, family
    )
}

/// 获取默认的增强prompt
pub fn get_default_enhanced_prompt() -> String {
    build_enhanced_prompt(CHAT_PROMPT)
}
