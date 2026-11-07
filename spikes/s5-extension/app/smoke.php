<?php

// CLI smoke test: runs three prepared statements and prints the
// extension's counters. Usage: php -d extension=... smoke.php

declare(strict_types=1);

$pdo = new PDO('sqlite::memory:', options: [PDO::ATTR_ERRMODE => PDO::ERRMODE_EXCEPTION]);
$pdo->exec('CREATE TABLE t (id INTEGER PRIMARY KEY, name TEXT)');
$insert = $pdo->prepare("INSERT INTO t (name) VALUES (?) /* \"quoted\"\n\tnext line */");
$insert->execute(['a']);
$insert->execute(['b']);
$select = $pdo->prepare('SELECT name FROM t WHERE id = ?');
$select->execute([2]);
echo $select->fetchColumn(), "\n";
try {
    $pdo->prepare('INSERT INTO t (id, name) VALUES (?, ?)')->execute([1, 'duplicate key']);
} catch (PDOException $e) {
    echo 'caught: ', $e->getMessage(), "\n";
}
echo function_exists('debug_claw_s5_stats') ? debug_claw_s5_stats() : 'extension not loaded', "\n";
