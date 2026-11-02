<?php

namespace Database\Seeders;

use App\Enums\PaymentMethod;
use App\Enums\SubscriptionPlanType;
use App\Models\Doctor;
use App\Models\Patient;
use App\Models\Service;
use App\Models\Setting;
use App\Models\Sponsor;
use App\Models\User;
use App\Services\DoctorPayoutService;
use App\Services\SessionService;
use App\Services\SubscriptionService;
use App\Services\WalletService;
use Illuminate\Database\Console\Seeds\WithoutModelEvents;
use Illuminate\Database\Seeder;

class DatabaseSeeder extends Seeder
{
    use WithoutModelEvents;

    /**
     * Seed the application's database.
     */
    public function run(): void
    {
        $admin = User::factory()->admin()->create([
            'name' => 'سالم الورفلي',
            'username' => 'admin',
            'password' => 'Admin123!',
        ]);
        $receptionist = User::factory()->create([
            'name' => 'سارة الرويفي',
            'username' => 'reception',
            'password' => 'Reception123!',
        ]);
        User::factory()->create([
            'name' => 'مريم الترهوني',
            'username' => 'reception2',
            'password' => 'Reception123!',
        ]);

        $sponsors = collect([
            ['name' => 'شركة المدار الجديد', 'phone' => '0911111101', 'address' => 'طرابلس'],
            ['name' => 'شركة ليبيا للتأمين', 'phone' => '0911111102', 'address' => 'بن عاشور'],
            ['name' => 'صندوق الرعاية الاجتماعية', 'phone' => '0911111103', 'address' => 'حي الأندلس'],
        ])->map(fn (array $data) => Sponsor::create([...$data, 'is_active' => true, 'created_by' => $admin->id]));

        $doctors = collect([
            ['name' => 'د. أحمد الشريف', 'phone' => '0922000001', 'commission_rate' => '30.000'],
            ['name' => 'د. مريم سالم', 'phone' => '0922000002', 'commission_rate' => '35.000'],
            ['name' => 'د. خالد المنصوري', 'phone' => '0922000003', 'commission_rate' => '25.000'],
        ])->map(fn (array $data) => Doctor::create([...$data, 'is_active' => true, 'created_by' => $admin->id]));

        $services = collect([
            ['name' => 'الحجامة', 'price' => '30.000', 'included_units' => 7, 'unit_name' => 'كوب', 'extra_unit_price' => '5.000'],
            ['name' => 'تمارين علاجية', 'price' => '40.000'],
            ['name' => 'علاج يدوي', 'price' => '70.000'],
            ['name' => 'تأهيل إصابات رياضية', 'price' => '100.000'],
            ['name' => 'علاج كهربائي', 'price' => '50.000'],
            ['name' => 'تأهيل ما بعد العمليات', 'price' => '120.000'],
            ['name' => 'علاج آلام الظهر', 'price' => '90.000'],
            ['name' => 'جلسة تقييم', 'price' => '60.000'],
        ])->map(fn (array $data) => Service::create([...$data, 'is_active' => true, 'created_by' => $admin->id]));

        $names = [
            'علي المصراتي', 'هدى الترهوني', 'خالد بن عمران', 'فاطمة العبيدي', 'عمر الزوي',
            'مريم السنوسي', 'أحمد القماطي', 'سلمى الدرسي', 'نبيل الفيتوري', 'سارة الشيباني',
            'محمد اللافي', 'آمنة الغرياني', 'يوسف البرعصي', 'نور الهادي', 'طارق المغربي',
            'إيمان الزنتاني', 'محمود العريبي', 'رنا الساحلي', 'وليد التاجوري', 'حنان المقريف',
        ];
        $patients = collect($names)->map(function (string $name, int $index) use ($admin, $sponsors): Patient {
            return Patient::create([
                'name' => $name,
                'phone' => '09'.str_pad((string) (10000000 + $index), 8, '0', STR_PAD_LEFT),
                'age' => 18 + ($index * 3) % 55,
                'gender' => $index % 2 === 0 ? 'male' : 'female',
                'area' => ['طرابلس', 'بن عاشور', 'حي الأندلس', 'سوق الجمعة'][$index % 4],
                'emergency_phone' => null,
                'sponsor_id' => $index % 5 === 0 ? $sponsors[$index % $sponsors->count()]->id : null,
                'is_active' => true,
                'created_by' => $admin->id,
            ]);
        });

        Setting::create([
            'clinic_name' => 'عيادة اتزان',
            'clinic_subtitle' => 'للعلاج الطبيعي والتأهيل',
            'phone' => '091-555-2026',
            'address' => 'طرابلس، ليبيا',
            'receipt_footer' => 'نتمنى لكم دوام الصحة والعافية',
            'currency' => 'د.ل',
            'print_size' => 'A4',
            'show_logo' => true,
            'updated_by' => $admin->id,
        ]);

        $walletService = app(WalletService::class);
        $subscriptionService = app(SubscriptionService::class);
        $sessionService = app(SessionService::class);

        $walletService->topUp($patients[0], '300.000', PaymentMethod::Cash, 'رصيد افتتاحي تجريبي', $receptionist);

        $subscriptionResult = $subscriptionService->create([
            'patient_id' => $patients[0]->id,
            'plan_type' => SubscriptionPlanType::Monthly->value,
            'amount' => '600.000',
            'paid_amount' => '500.000',
            'method' => PaymentMethod::Transfer->value,
            'start_date' => today()->subMonth()->toDateString(),
            'end_date' => today()->addDays(10)->toDateString(),
            'sponsor_id' => $sponsors[0]->id,
        ], $receptionist);

        $sessionService->createAndClose([
            'patient_id' => $patients[0]->id,
            'doctor_id' => $doctors[0]->id,
            'type' => 'subscription',
            'subscription_id' => $subscriptionResult['subscription']->id,
            'services' => [
                ['service_id' => $services[0]->id, 'extra_units' => 2],
            ],
        ], $receptionist);

        $sessionService->createAndClose([
            'patient_id' => $patients[1]->id,
            'doctor_id' => $doctors[0]->id,
            'type' => 'individual',
            'service_ids' => [$services[0]->id],
            'discount' => '0.000',
            'settlement' => ['paid_amount' => '30.000', 'wallet_amount' => '0.000', 'debt_amount' => '0.000', 'method' => 'cash'],
        ], $receptionist);

        $sessionService->createAndClose([
            'patient_id' => $patients[0]->id,
            'doctor_id' => $doctors[1]->id,
            'type' => 'individual',
            'service_ids' => [$services[2]->id],
            'discount' => '0.000',
            'settlement' => ['paid_amount' => '0.000', 'wallet_amount' => '70.000', 'debt_amount' => '0.000'],
        ], $receptionist);

        $sessionService->createAndClose([
            'patient_id' => $patients[2]->id,
            'doctor_id' => $doctors[2]->id,
            'type' => 'individual',
            'service_ids' => [$services[3]->id],
            'discount' => '0.000',
            'settlement' => ['paid_amount' => '0.000', 'wallet_amount' => '0.000', 'debt_amount' => '100.000', 'sponsor_id' => $sponsors[1]->id],
        ], $receptionist);

        $sessionService->createAndClose([
            'patient_id' => $patients[0]->id,
            'doctor_id' => $doctors[1]->id,
            'type' => 'individual',
            'service_ids' => [$services[0]->id, $services[1]->id],
            'discount' => '0.000',
            'settlement' => ['paid_amount' => '20.000', 'wallet_amount' => '20.000', 'debt_amount' => '30.000', 'method' => 'transfer'],
        ], $receptionist);

        foreach ([3, 4, 5] as $index) {
            $sessionService->createAndClose([
                'patient_id' => $patients[$index]->id,
                'doctor_id' => $doctors[$index % $doctors->count()]->id,
                'type' => 'individual',
                'service_ids' => [$services[$index % $services->count()]->id],
                'discount' => '0.000',
                'settlement' => [
                    'paid_amount' => $services[$index % $services->count()]->price,
                    'wallet_amount' => '0.000',
                    'debt_amount' => '0.000',
                    'method' => 'cash',
                ],
            ], $receptionist);
        }

        $payoutService = app(DoctorPayoutService::class);
        $payoutService->create($doctors[0], '20.000', today()->toDateString(), PaymentMethod::Cash, 'دفعة تجريبية', $admin);
    }
}
