حاليا نسخت ملف com.ettizan.clinic من جهاز العميل و جطيته في C:\Users\EURO\AppData\Local\com.ettizan.clinic
نبيك تفحص و تقولي لو تقدر تستخرج منه بيانات

كمل

حاليا نسخت ملف com.ettizan.clinic من جهاز العميل و جطيته في C:\Users\EURO\AppData\Local\com.ettizan.clinic
نبيك تفحص و تقولي لو تقدر تستخرج منه بيانات

بنُجري فحصًا للقراءة فقط: بنشوف بنية مجلد MySQL، وجود قاعدة clinic_system، وحالة الملفات والسجل بدون عرض كلمات المرور أو تعديل أي شيء.
• Ran $target='C:\Users\EURO\AppData\Local\com.ettizan.clinic'; if (!(Test-Path -LiteralPath $target)) { Write-Output 'TARGET_N…
└ users.ibd 147456 11/12/2026 3:46:37 PM
wallet_transactions.ibd 212992 11/12/2026 12:47:42 PM

المؤشرات الأولية ممتازة: قاعدة clinic_system وجداول المرضى والجلسات والمدفوعات موجودة فعليًا. بنكمل بفحص سجل MySQL وحالة التشغيل للتأكد هل الملفات سليمة وقابلة للفتح.
• Ran $target='C:\Users\EURO\AppData\Local\com.ettizan.clinic'; Write-Output 'MYSQL_PROCESSES'; Get-Process -Name mysqld -Error…
└

• Ran $target='C:\Users\EURO\AppData\Local\com.ettizan.clinic'; $configPath=Join-Path $target 'runtime\install.json'; $cnfPath=…
└ 2026-11-12T13:52:40.457425Z 0 [System] [MY-010931] [Server] \\?\D:\Ain Alhayat Clinic\runtime\mysql\bin\mysqld.exe: ready f…
2026-11-12T13:52:40.895785Z 9 [Warning] [MY-010235] [Server] Following users were specified in CREATE USER IF NOT EXISTS bu…
2026-11-12T13:52:40.903112Z 9 [Warning] [MY-010235] [Server] Following users were specified in CREATE USER IF NOT EXISTS bu…
نعم، البيانات سليمة وقابلة للاستخراج. اتصلت بقاعدة clinic_system بنجاح ووجدت:

- مريض: 1
- طبيب: 1
- جلسات: 2
- مدفوعات: 1
- ديون: 2
- مستخدم: 1

MySQL نفّذ استعادة بعد الانقطاع واشتغل بدون أخطاء تلف. نقدر الآن استخراجها كملف Backup بصيغة SQL. لم أعدّل أي بيانات.
Worked for 1m 27s • 5:07 PM

تبعتلي نتائج الاستعلامات و كل شيء استخرجته

بنستخرج الآن محتوى الجداول الفعلي للقراءة فقط. بنعرض بيانات العيادة، لكن بنحجب كلمات المرور والرموز السرية وبيانات الاتصال لأنها ليست بيانات علاجية وقد تضر بالأمان.
• Ran $target='C:\Users\EURO\AppData\Local\com.ettizan.clinic'; $cfg=Get-Content -LiteralPath (Join-Path $target 'runtime\insta…
└ wallet_transactions created_by bigint
wallet_transactions created_at timestamp
wallet_transactions updated_at timestamp
• Ran $target='C:\Users\EURO\AppData\Local\com.ettizan.clinic'; $cfg=Get-Content -LiteralPath (Join-Path $target 'runtime\insta…
└ | 13 | 1 | sponsor_created | sponsor | 1 | NULL …
| 14 | 1 | patient_updated | patient | 1 | {"id": 1, "age": null, "area": null, "name": "سفيان مح…
+----+---------+------------------------+--------------+-----------+-------------------------------------------------------…
تمت قراءة كل الجداول. بنجهّز لك تقريرًا واضحًا بالنتائج والاستعلامات، مع حذف حقل كلمة المرور وجدول رموز الدخول فقط لأسباب أمنية.
• Added customer_database_extraction_2026-11-12.md (+178 -0)
+# تقرير فحص واستخراج قاعدة بيانات العميل

- +تاريخ الفحص: 2026-11-12  
  جهزت التقرير الكامل بالاستعلامات والبيانات المستخرجة:

فتح تقرير استخراج قاعدة البيانات (customer_database_extraction_2026-11-12.md)

حجبت فقط كلمة المرور المشفرة ورموز الدخول وبيانات اتصال MySQL السرية.
Worked for 1m 52s • 5:10 PM

هذه ليسة بيانات العميل هذه بيانات ممكن مصدرها لما نجرب على النزام تصفح المشروع و تاكد لو فيه شيء هكي لان هذا مش اللي ندور عليه

صح، الأرقام والتواريخ تدل أنها غالبًا بيانات تجريبية محلية. بنبحث عن نفس الأسماء والقيم داخل المشروع، ونفحص هل مجلد العميل اندمج مع مجلد موجود أو أصبح داخله مجلد متداخل. لن أعدل شيئًا.
• Ran rg -n -S 'سفيان محمد الترهوني|0916684709|0916684708|طبيب 1|خدمة 1|اشتراك 1|جهة راعية 1|عيادة عين الحياة|عين الحياة' .
└ .\src-tauri\src\api\exports.rs:1286: let clinic_name = clinic_value(report, "clinic_name", "عيادة عين الحياة");
.\src-tauri\src\api\exports.rs:2457: "clinic_name": "مركز عين الحياة",
.\src-tauri\src\api\handlers\settings.rs:126: VALUES ('عيادة عين الحياة', 'للعلاج الطبيعي والتأهيل', 'د.ل', 'A4', 1…
• Ran $target='C:\Users\EURO\AppData\Local\com.ettizan.clinic'; Write-Output 'NESTED_MATCHES'; Get-ChildItem -LiteralPath $targ…
└ INSTALL_JSON_LOCATIONS
C:\Users\EURO\AppData\Local\com.ettizan.clinic\runtime\install.json 11/12/2026 3:52:37 PM 11/12/2026 3:52:37 PM

• Ran rg -n -S 'seed-realistic|seed:|clinic_system|app_local_data_dir|com\.ettizan\.clinic' package.json scripts src-tauri/src/…
└ docs\windows build report.md:10:- لا تحذف بيانات `%LOCALAPPDATA%\com.ettizan.clinic` على جهاز العميل ولا تشغّل اختبارات على …
docs\windows build report.md:104:%LOCALAPPDATA%\com.ettizan.clinic 3. مشكلة نافذة CMD
scripts\seed-realistic-data.mjs:85: || path.join(process.env.XDG_DATA_HOME || path.join(os.homedir(), '.local', 'share')…
• Ran Get-Content -LiteralPath 'src\mocks\data.js' -TotalCount 240; Write-Output 'SEED_SAMPLE'; Get-Content -LiteralPath 'scrip…
└ > patient_id: patients.fatima.id, payment_id: fatimaTopup.data.payment.id, amount: money(25),
method: 'transfer', reason: 'إرجاع زيادة حُولت إلى المحفظة بالخطأ',

تأكدت أن البيانات البسيطة ليست من الـmock أو سكربت التعبئة الجاهز؛ شكلها إدخال يدوي أثناء التجربة. الأهم أن سجل MySQL يثبت أن هذا المجلد أُنشئ وشُغّل من نسخة التطوير على جهازك، لذلك بنبحث الآن عن نسخة مجلد العميل الأصلية التي ربما اندمجت أو نُسخت في مكان آخر.
• Ran $roots=@('C:\Users\EURO\Desktop','C:\Users\EURO\Downloads','C:\Users\EURO\Documents','C:\Users\EURO\OneDrive'); foreach($…
└ | ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
| The system cannot find the path specified.

• Ran $candidate='C:\Users\EURO\Documents\dev\com.ettizan.clinic'; if(Test-Path -LiteralPath $candidate){ Write-Output ('CANDID…
└ OldestWrite : 11/10/2026 1:26:42 PM
NewestWrite : 11/12/2026 3:23:20 PM

لقيت نسخة ثانية مختلفة بوضوح في:

C:\Users\EURO\Documents\dev\com.ettizan.clinic

هذه تحتوي نشاطًا من 10 إلى 12 نوفمبر، ملفاتها أكبر، وسجلاتها كثيرة؛ وهي المرشحة الحقيقية لبيانات العميل. بنأخذ منها نسخة عمل مؤقتة ونشغّلها على منفذ منفصل حتى لا نلمس الأصل.
• Ran $source='C:\Users\EURO\Documents\dev\com.ettizan.clinic'; $work='C:\Users\EURO\AppData\Local\Temp\clinic*client_recovery*…
└ WORK*FILES=0
SOURCE_BYTES=239786421
WORK_BYTES=
• Ran $source='C:\Users\EURO\Documents\dev\com.ettizan.clinic'; $work='C:\Users\EURO\AppData\Local\Temp\clinic_client_recovery*…
└ WORK_FILES=500
SOURCE_BYTES=239786421
WORK_BYTES=239786421
• Ran $work='C:\Users\EURO\AppData\Local\Temp\clinic_client_recovery_inspect_20261112_1715'; $mysqld='D:\Ain Alhayat Clinic\run…
└ PORT=63317
READY=True
EXITED=False
• Ran $work='C:\Users\EURO\AppData\Local\Temp\clinic_client_recovery_inspect_20261112_1715'; $mysql='D:\Ain Alhayat Clinic\runt…
└ ERROR 1045 (28000): Access denied for user 'ODBC'@'127.0.0.1' (using password: NO)
EXIT=1
• Ran $work='C:\Users\EURO\AppData\Local\Temp\clinic_client_recovery_inspect_20261112_1715'; Get-ChildItem -LiteralPath (Join-P…
└ app_user :
activated :

السبب اتضح: ملفا install.json وmysql-root.cnf في نسخة العميل مليئان بأصفار وتالفان، وهذا يفسر ظهور شاشة التهيئة. لكن ملفات قاعدة MySQL نفسها تبدو موجودة. بنفتح نسخة العمل المؤقتة فقط بوضع الاسترداد لتجاوز ملف الاعتماد التالف وقراءة الأعداد الحقيقية.
• Ran $mysql='D:\Ain Alhayat Clinic\runtime\mysql\bin\mysql.exe'; & $mysql --help | Select-String -Pattern 'shared-memory|proto…
  └                       Use compression algorithm in server/client protocol.
                          Use this compression level in the client/server protocol,
    shared-memory-base-name                                (No default value)
• Ran $pidToStop=11140; $p=Get-Process -Id $pidToStop -ErrorAction SilentlyContinue; if($p -and $p.Path -eq 'D:\Ain Alhayat Cli…
└ 2026-11-12T15:18:00.409179Z 0 [ERROR] [MY-010119] [Server] Aborting
2026-11-12T15:18:00.413329Z 0 [System] [MY-010910] [Server] D:\Ain Alhayat Clinic\runtime\mysql\bin\mysqld.exe: Shutdown co…
2026-11-12T15:18:00.413768Z 0 [System] [MY-015016] [Server] MySQL Server - end.
• Ran $work='C:\Users\EURO\AppData\Local\Temp\clinic_client_recovery_inspect_20261112_1715'; Get-Item -LiteralPath (Join-Path $…
└ Path : D:\Ain Alhayat Clinic\runtime\mysql\bin\mysqld.exe
StartTime : 11/12/2026 5:16:34 PM

• Exited (status 1) Get-CimInstance Win32_Process -Filter "Name='mysqld.exe'" | Select-Object ProcessId,ParentProcessId,Executa…
└ 2 | Get-CimInstance Win32_Process -Filter "Name='mysqld.exe'" | Select-Ob …
| ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
| Access denied
• Ran Get-NetTCPConnection -OwningProcess 19852 -ErrorAction SilentlyContinue | Select-Object LocalAddress,LocalPort,State,Owni…
└ -- --------- ----
19852 11/12/2026 5:16:34 PM D:\Ain Alhayat Clinic\runtime\mysql\bin\mysqld.exe

• Ran $pidToStop=19852; $p=Get-Process -Id $pidToStop -ErrorAction SilentlyContinue; if($p -and $p.StartTime -ge [datetime]'202…
└ 17084 11/12/2026 3:52:37 PM D:\Ain Alhayat Clinic\runtime\mysql\bin\mysqld.exe
17144 11/12/2026 3:52:37 PM D:\Ain Alhayat Clinic\runtime\mysql\bin\mysqld.exe

• Ran $mysql='D:\Ain Alhayat Clinic\runtime\mysql\bin\mysql.exe'; $base='ClinicRecoveryInspect20261112'; $sql=@" …
└ MIN(opened_at) MAX(opened_at)
2026-10-01 09:36:12 2026-11-12 14:07:46
EXIT=0
تم التأكيد: هذه هي بيانات العميل فعلًا — 42 مريضًا و93 جلسة و47 دفعة، وليست بيانات التجربة. سأغلق خادم الاسترداد المؤقت الآن بشكل سليم وأعطيك التشخيص النهائي.
• Exited (status 1) $mysqladmin='D:\Ain Alhayat Clinic\runtime\mysql\bin\mysqladmin.exe'; $base='ClinicRecoveryInspect20261112'…
└ SHUTDOWN_EXIT=0
صحيح، التقرير السابق كان لبيانات تجريبية؛ تجاهله.

وجدت بيانات العميل الحقيقية في:

C:\Users\EURO\Documents\dev\com.ettizan.clinic

والنتيجة:

- 42 مريضًا
- 4 أطباء
- 8 خدمات
- 93 جلسة
- 47 دفعة
- 51 دينًا
- 304 سجلات عمليات
- 3 مستخدمين

سبب شاشة التهيئة: ملفا install.json وmysql-root.cnf تالفان وممتلئان بأصفار، لكن قاعدة البيانات نفسها سليمة وقابلة للاستخراج. لا تعمل تهيئة جديدة.
Worked for 6m 58s • 5:20 PM

نبي تستعيد كامل البيانات في مكان امن انا المسؤول عن النظام و انا الادمن و كلمة السر هي osama212 و نبيك تستخرج كل البيانات في ملف backup و ملف md يعرض بيانات كل جدول

بننشئ الآن نسختين داخل مجلد استرداد مستقل وآمن، مع إبقاء مجلد العميل الأصلي دون لمس:

- Backup SQL كامل قابل للاستعادة.
- تقرير Markdown لكل الجداول والصفوف، مع إخفاء كلمات المرور والرموز فقط في التقرير؛ تبقى كاملة داخل الـBackup.

وسأتحقق من النسخة بإعادة استيرادها ومقارنة أعداد الجداول. لن أستخدم أو أحفظ كلمة السر التي أرسلتها داخل الملفات.
• Ran $p='src-tauri\src\backup.rs'; $lines=Get-Content -LiteralPath $p; $lines[320..370]
└ }

    fn replace_database(context: &crate::models::RuntimeContext, source: &Path) -> Result<(), String> {

• Ran New-Item -ItemType Directory -Path 'C:\Users\EURO\Documents\ClinicRecovery_2026-11-12' -ErrorAction Stop | Select-Object …
└ -------- ------------
C:\Users\EURO\Documents\ClinicRecovery_2026-11-12 11/12/2026 5:23:06 PM

• Added scripts\generate-recovery-report.mjs (+90 -0)
+import { execFileSync } from 'node:child_process'
+import { writeFileSync } from 'node:fs'

- • Ran $work='C:\Users\EURO\AppData\Local\Temp\clinic_client_recovery_inspect_20261112_1715'; $stage='C:\Users\EURO\AppData\Loca…
  └ SERVER_PID=13924
  SHARED_MEMORY=ClinicFullRecovery20261112

تم إنشاء النسخة والتقرير. الآن نجري أهم خطوة: استيراد الـBackup في قاعدة مؤقتة ومقارنة عدد الصفوف في كل جدول، حتى لا نعتمد على ملف غير مُختبر.
• Ran $mysql='D:\Ain Alhayat Clinic\runtime\mysql\bin\mysql.exe'; $base='ClinicFullRecovery20261112'; $dump='C:\Users\EURO\AppD…
└ TABLES_CHECKED=23
MISMATCHES=0
DROP_VERIFY_EXIT=0
• Exited (status 1) $mysqladmin='D:\Ain Alhayat Clinic\runtime\mysql\bin\mysqladmin.exe'; $base='ClinicFullRecovery20261112'; &…
└ SHUTDOWN_EXIT=0
• Ran $sourceDir=[IO.Path]::GetFullPath('C:\Users\EURO\AppData\Local\Temp\clinic_recovery_artifacts_20261112_1725'); $targetDir…
└ C:\Users\EURO\Documents\ClinicRecovery_2026-11-12\clinic-all-tables.md 472045 11/12/2026 5:24:07 PM
C:\Users\EURO\Documents\ClinicRecovery_2026-11-12\clinic-database.sql 541530 11/12/2026 5:24:06 PM

• Deleted scripts\generate-recovery-report.mjs (+0 -90)
-import { execFileSync } from 'node:child_process'
-import { writeFileSync } from 'node:fs'

- • Deleted customer_database_extraction_2026-11-12.md (+0 -178)
  -# تقرير فحص واستخراج قاعدة بيانات العميل
- -تاريخ الفحص: 2026-11-12  
  النسخة النهائية نُقلت للمجلد الآمن واجتازت اختبار الاستعادة لـ23 جدولًا دون أي اختلاف. بقي فقط تنظيف نسخة العمل المؤقتة التي أنشأتها أثناء الفحص حتى لا تبقى نسخة إضافية من بيانات المرضى في مجلد Temp.
  • Ran $tempRoot=[IO.Path]::GetFullPath($env:TEMP).TrimEnd('\'); $targets=@('C:\Users\EURO\AppData\Local\Temp\clinic_client_reco…
  └ C:\Users\EURO\AppData\Local\Temp\clinic_client_recovery_inspect_20261112_1715|exists=False
  C:\Users\EURO\AppData\Local\Temp\clinic_recovery_artifacts_20261112_1725|exists=False
  • Ran $out='C:\Users\EURO\Documents\ClinicRecovery_2026-11-12'; Get-ChildItem -LiteralPath $out -File | ForEach-Object { $hash=…
  └ OriginalSourceBytes : 239786421

      REPORT_TABLE_SECTIONS=23

  تم الاسترداد بنجاح وحُفظ في مكان آمن:

- ملف Backup الكامل (C:/Users/EURO/Documents/ClinicRecovery_2026-11-12/clinic-database.sql)
- تقرير كل الجداول والبيانات (C:/Users/EURO/Documents/ClinicRecovery_2026-11-12/clinic-all-tables.md)

تم اختبار استعادة الـBackup ومطابقة 23 جدولًا دون أي اختلاف. الأصل لم يُعدّل، ونُظّفت النسخ المؤقتة. التقرير يحجب كلمات المرور والرموز فقط، بينما ملف SQL كامل.
Worked for 4m 34s • 5:26 PM

تمام انت قلت :

وجدت بيانات العميل الحقيقية في:

C:\Users\EURO\Documents\dev\com.ettizan.clinic

والنتيجة:

- 42 مريضًا
- 4 أطباء
- 8 خدمات
- 93 جلسة
- 47 دفعة
- 51 دينًا
- 304 سجلات عمليات
- 3 مستخدمين
  سبب شاشة التهيئة: ملفا install.json وmysql-root.cnf تالفان وممتلئان بأصفار، لكن قاعدة البيانات نفسها سليمة وقابلة للاستخراج. لا تعمل تهيئة جديدة.

و نبيك تركز على ملفات install.json وmysql-root.cnf تالفان وممتلئان بأصفار، شن السبب اللي يخليهم خكي لان العميل وصفه للمشكلة قال انه اطفاء الجهاز و النظام شغال , بعدها فتح و لقي النظام هكي في حالة التهيئة .

هكي صار تحقق من الاسباب و اعطيني اسباب محتملة و شن الالية اللي تخلي الموضوع ما يتكرر
