// Utility functions for formatting data
// This file contains common formatting functions used throughout the application

export function formatDate(date: Date): string {
  return date.toLocaleDateString();
}

export function formatCurrency(amount: number, currency: string = 'USD'): string {
  return new Intl.NumberFormat('en-US', {
    style: 'currency',
    currency: currency,
  }).format(amount);
}

export function generateId(): string {
  return crypto.randomUUID();
}
