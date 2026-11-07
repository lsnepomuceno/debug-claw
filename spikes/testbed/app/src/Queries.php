<?php

declare(strict_types=1);

namespace App;

use PDO;

/** Runs about $n PDO queries against an in-memory SQLite database. */
final class Queries
{
    public function run(int $n): array
    {
        $pdo = new PDO('sqlite::memory:', options: [PDO::ATTR_ERRMODE => PDO::ERRMODE_EXCEPTION]);
        $pdo->exec('CREATE TABLE products (id INTEGER PRIMARY KEY, name TEXT, price REAL)');

        $insert = $pdo->prepare('INSERT INTO products (name, price) VALUES (?, ?)');
        $pdo->beginTransaction();
        for ($i = 1; $i <= 50; $i++) {
            $insert->execute(["product-$i", $i * 1.5]);
        }
        $pdo->commit();

        $select = $pdo->prepare('SELECT name, price FROM products WHERE id = ?');
        $total = 0.0;
        for ($i = 0; $i < $n; $i++) {
            $select->execute([($i % 50) + 1]);
            $row = $select->fetch(PDO::FETCH_ASSOC);
            $total += $row['price'];
        }

        return ['queries' => $n, 'total' => $total];
    }
}
