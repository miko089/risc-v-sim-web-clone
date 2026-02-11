use anyhow::Result;
use risc_v_sim_web::database::{DatabaseService, SubmissionRecord, SubmissionStatus};
use serde_json;
use std::env;
use mongodb::bson;
use ulid::Ulid;
use bson::DateTime;

#[tokio::main]
async fn main() -> Result<()> {
    let db_service = DatabaseService::new().await?;

    let test_user_ids = vec![
        // miko089's GitHub user id for me to be able to see my submissions even in test run
        75020830i64,
        98765432i64,
        55566677i64,
        11122233i64,
    ];

    for (index, user_id) in test_user_ids.iter().enumerate() {
        println!("Creating submissions for user ID: {}", user_id);

        let num_submissions = if index == 0 {
            8
        } else {
            rand::random::<usize>() % 6 + 3
        };

        for i in 0..num_submissions {
            let uuid = Ulid::new().to_string();
            let status = match i % 3 {
                0 => SubmissionStatus::Completed,
                1 => SubmissionStatus::InProgress,
                _ => SubmissionStatus::Awaits,
            };

            let hours_ago = (i * 2 + rand::random::<usize>() % 24) as i64;
            let now_millis = DateTime::now().timestamp_millis();
            let created_at = DateTime::from_millis(now_millis - hours_ago * 3_600_000);
            let updated_at = if matches!(status, SubmissionStatus::Completed) {
                let minutes_offset = (rand::random::<i64>().abs() % 120) * 60_000;
                DateTime::from_millis(created_at.timestamp_millis() + minutes_offset)
            } else {
                created_at
            };

            let submission = SubmissionRecord {
                id: None,
                uuid: uuid.clone(),
                user_id: *user_id,
                status,
                created_at,
                updated_at,
            };

            let inserted_id = db_service.create_submission(submission).await?;
            println!("  Created submission {} with ID: {}", uuid, inserted_id);

            create_submission_files(&uuid, i).await?;
        }
    }

    println!("Database population completed!");
    Ok(())
}

async fn create_submission_files(uuid: &str, index: usize) -> Result<()> {
    // TODO: use files instead of hard-coded samples

    let submissions_dir =
        env::var("SUBMISSIONS_FOLDER").unwrap_or_else(|_| "submission".to_string());
    let submission_path = format!("{}/{}", submissions_dir, uuid);

    tokio::fs::create_dir_all(&submission_path).await?;

    let sample_codes = vec![
        r#"
        .text
        .globl _start

    _start:
        li x5, 10
        li x6, 20
        add x7, x5, x6
        ebreak"#,
        r#"
        .text
        .globl _start

    _start:
        li x5, 0
        li x6, 5

    loop:
        addi x5, x5, 1
        bne x5, x6, loop
        ebreak"#,
        r#"
        .text
        .globl _start

    _start:
        la x10, data
        li x11, 42
        sw x11, 0(x10)
        lw x12, 0(x10)
        ebreak

    .data
    data: .word 0"#,
        r#"
        .text
        .globl _start

    _start:
        li x5, 0
        li x6, 1
        li x7, 10

    fib_loop:
        add x8, x5, x6
        mv x5, x6
        mv x6, x8
        addi x7, x7, -1
        bnez x7, fib_loop
        ebreak"#,
        r#"
        .text
        .globl _start

    _start:
        li x5, 100
        li x6, 25
        sub x7, x5, x6
        li x8, 3
        mul x9, x7, x8
        ebreak"#,
    ];

    let code_index = index % sample_codes.len();
    let assembly_code = sample_codes[code_index];

    let input_file = format!("{}/input.s", submission_path);
    tokio::fs::write(&input_file, assembly_code).await?;

    if index % 3 == 0 {
        let simulation_result = create_sample_simulation_result(&assembly_code, index);
        let result_file = format!("{}/simulation.json", submission_path);
        tokio::fs::write(result_file, simulation_result).await?;
    }

    Ok(())
}

fn create_sample_simulation_result(assembly_code: &str, index: usize) -> String {
    // TODO: use files instead of hard-coded samples

    let ulid = Ulid::new();
    let steps = if index % 3 == 0 {
        vec![
            serde_json::json!({
                "instruction": {
                    "mnemonic": "li",
                    "obj": {"Li": [5, 10]}
                },
                "old_registers": {
                    "pc": 1342177280,
                    "storage": [0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
                },
                "new_registers": {
                    "pc": 1342177284,
                    "storage": [0, 0, 0, 0, 0, 10, 0, 0, 0, 0]
                }
            }),
            serde_json::json!({
                "instruction": {
                    "mnemonic": "li",
                    "obj": {"Li": [6, 20]}
                },
                "old_registers": {
                    "pc": 1342177284,
                    "storage": [0, 0, 0, 0, 0, 10, 0, 0, 0, 0]
                },
                "new_registers": {
                    "pc": 1342177288,
                    "storage": [0, 0, 0, 0, 0, 10, 20, 0, 0, 0]
                }
            }),
            serde_json::json!({
                "instruction": {
                    "mnemonic": "add",
                    "obj": {"Add": [7, 5, 6]}
                },
                "old_registers": {
                    "pc": 1342177288,
                    "storage": [0, 0, 0, 0, 0, 10, 20, 0, 0, 0]
                },
                "new_registers": {
                    "pc": 1342177292,
                    "storage": [0, 0, 0, 0, 0, 10, 20, 30, 0, 0]
                }
            }),
        ]
    } else {
        vec![]
    };

    let result = serde_json::json!({
        "ulid": ulid.to_string(),
        "ticks": 10,
        "code": assembly_code,
        "steps": steps,
        "final_registers": {
            "pc": 1342177292,
            "storage": [0, 0, 0, 0, 0, 10, 20, 30, 0, 0]
        }
    });

    result.to_string()
}
