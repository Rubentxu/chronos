package com.chronos;

public class Hello {
    private static volatile int counter = 0;

    public static void main(String[] args) {
        System.out.println("Java server starting on :8080");

        Thread counterThread = new Thread(() -> {
            while (!Thread.currentThread().isInterrupted()) {
                counter++;
                if (counter % 1_000_000 == 0) {
                    try {
                        Thread.sleep(100);
                    } catch (InterruptedException e) {
                        Thread.currentThread().interrupt();
                        break;
                    }
                }
            }
        });
        counterThread.start();

        try {
            Thread.currentThread().join();
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
        }
    }
}
