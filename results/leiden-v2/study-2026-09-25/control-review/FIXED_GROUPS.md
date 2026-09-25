# Fixed-anchor groups

The 18 anchors were fixed before screening. Centers and boundaries use saved per-word geometry; the full member lists are in [the CSV](fixed-anchor-review.csv). These are contextual groups, not synonym lists.

## 苹果

| Partition | Size | Centers | Fixed sample | Boundary |
|---|---:|---|---|---|
| archived recommendation | 1 | 苹果 | 苹果 | 苹果 |
| seeded baseline | 4 | 新款 系列 问世 苹果 | 苹果 问世 系列 新款 | 苹果 系列 问世 新款 |
| control t=0.7 unshifted q=0.3 | 4 | 新款 系列 问世 苹果 | 苹果 问世 系列 新款 | 系列 苹果 问世 新款 |

## 牛奶

| Partition | Size | Centers | Fixed sample | Boundary |
|---|---:|---|---|---|
| archived recommendation | 165 | 米饭 豆腐 香肠 豆浆 卤味 饺子 薯条 酸奶 | 蘸 茶叶 饮料 豆浆 橙汁 炖 咖啡 馒头 | 啤酒 味道 电饭锅 晚餐 桂花 碗 |
| seeded baseline | 164 | 米饭 豆腐 香肠 卤味 豆浆 饺子 薯条 酸奶 | 蘸 茶叶 饮料 豆浆 橙汁 炖 咖啡 馒头 | 味道 啤酒 香味 年夜饭 晚餐 桂花 |
| control t=0.7 unshifted q=0.3 | 157 | 米饭 豆腐 豆浆 香肠 卤味 薯条 酸奶 饺子 | 蘸 茶叶 饮料 豆浆 橙汁 炖 咖啡 馒头 | 味道 腥 啤酒 香味 晚餐 桂花 |

## 咖啡

| Partition | Size | Centers | Fixed sample | Boundary |
|---|---:|---|---|---|
| archived recommendation | 165 | 米饭 豆腐 香肠 豆浆 卤味 饺子 薯条 酸奶 | 饭 调料 味精 桂花 热腾腾 火锅 茄子 绿茶 | 啤酒 味道 电饭锅 晚餐 桂花 碗 |
| seeded baseline | 164 | 米饭 豆腐 香肠 卤味 豆浆 饺子 薯条 酸奶 | 饭 调料 味精 桂花 热腾腾 火锅 茄子 绿茶 | 味道 啤酒 香味 年夜饭 晚餐 桂花 |
| control t=0.7 unshifted q=0.3 | 157 | 米饭 豆腐 豆浆 香肠 卤味 薯条 酸奶 饺子 | 饭 调料 味精 桂花 热腾腾 火锅 茄子 绿茶 | 味道 腥 啤酒 香味 晚餐 桂花 |

## 衬衫

| Partition | Size | Centers | Fixed sample | Boundary |
|---|---:|---|---|---|
| archived recommendation | 81 | 衬衣 毛衣 大衣 衬衫 衣服 袜子 裙子 裤子 | 地毯 上衣 窗帘 雨衣 毛巾 书包 打扮 穿 | 穿 帽子 口罩 绳子 手套 铅笔 |
| seeded baseline | 71 | 衬衣 衬衫 毛衣 大衣 裙子 上衣 外套 衣服 | 地毯 上衣 窗帘 雨衣 毛巾 书包 打扮 穿 | 铅笔 纱 时装 丝绸 首饰 打扮 |
| control t=0.7 unshifted q=0.3 | 68 | 衬衣 衬衫 毛衣 大衣 裙子 上衣 外套 衣服 | 地毯 上衣 雨衣 毛巾 书包 打扮 穿 裤子 | 铅笔 丝绸 首饰 时装 透气 拉锁 |

## 足球

| Partition | Size | Centers | Fixed sample | Boundary |
|---|---:|---|---|---|
| archived recommendation | 40 | 体操 乒乓球 田径 篮球 网球 羽毛球 排球 举重 | 跳远 太极拳 武术 帆船 球拍 健身 棒球 跳伞 | 太极拳 帆船 教练 体育 跳伞 武术 |
| seeded baseline | 49 | 乒乓球 田径 体操 网球 羽毛球 举重 排球 篮球 | 跳远 太极拳 赛场 奥运会 联赛 武术 帆船 球拍 | 太极拳 球队 帆船 武术 联赛 登山 |
| control t=0.7 unshifted q=0.3 | 40 | 体操 乒乓球 田径 篮球 羽毛球 网球 排球 举重 | 跳远 太极拳 武术 帆船 球拍 健身 跑步 棒球 | 太极拳 教练 武术 帆船 登山 健身 |

## 老师

| Partition | Size | Centers | Fixed sample | Boundary |
|---|---:|---|---|---|
| archived recommendation | 140 | 姑姑 舅舅 姐姐 妹妹 爸爸 嫂子 哥哥 奶奶 | 胖子 宝宝 女儿 太太 班长 家里 全家 岳母 | 亲属 老王 她们 战友 宝宝 朋友 |
| seeded baseline | 139 | 姑姑 舅舅 姐姐 妹妹 爸爸 嫂子 哥哥 奶奶 | 胖子 宝宝 女儿 太太 班长 家里 全家 岳母 | 老王 别人 她们 亲属 女生 朋友 |
| control t=0.7 unshifted q=0.3 | 134 | 姑姑 舅舅 姐姐 妹妹 嫂子 爸爸 哥哥 奶奶 | 胖子 宝宝 女儿 太太 家里 全家 岳母 母子 | 亲属 老王 朋友 男人 别人 贪玩儿 |

## 焦虑

| Partition | Size | Centers | Fixed sample | Boundary |
|---|---:|---|---|---|
| archived recommendation | 175 | 沮丧 羞愧 气愤 诧异 惊讶 惋惜 难过 心疼 | 凄凉 心疼 失望 钦佩 害臊 惭愧 妒忌 发怒 | 心里 反感 过意不去 自卑 惊叹 发脾气 |
| seeded baseline | 167 | 沮丧 羞愧 气愤 诧异 惊讶 惋惜 难过 吃惊 | 凄凉 心疼 失望 钦佩 害臊 振奋 惭愧 妒忌 | 反感 赞叹 过意不去 不已 遗憾 头疼 |
| control t=0.7 unshifted q=0.3 | 159 | 沮丧 羞愧 气愤 惊讶 诧异 惋惜 难过 惭愧 | 凄凉 心疼 失望 钦佩 害臊 振奋 惭愧 妒忌 | 赞叹 不已 介意 自卑 惊叹 怨气 |

## 政策

| Partition | Size | Centers | Fixed sample | Boundary |
|---|---:|---|---|---|
| archived recommendation | 11 | 思路 理念 方针 构想 举措 策略 战略 方案 | 策略 战略 方针 方案 思路 理念 决策 政策 | 方针 方案 理念 决策 政策 战略 |
| seeded baseline | 9 | 制度 体制 机制 体系 健全 流程 政策 改革 | 流程 体系 健全 程序 政策 体制 改革 机制 | 改革 政策 健全 流程 程序 体系 |
| control t=0.7 unshifted q=0.3 | 22 | 制定 制订 实施 颁布 出台 施行 实行 推行 | 规定 采取 草案 落实 拟定 修订 实施 颁布 | 拟定 草案 起草 政策 条例 采取 |

## 合同

| Partition | Size | Centers | Fixed sample | Boundary |
|---|---:|---|---|---|
| archived recommendation | 16 | 协议 签订 签署 协议书 订立 协定 合同 达成 | 达成 协议 约定 谅解 生效 契约 签署 签 | 签字 生效 条约 谅解 约定 契约 |
| seeded baseline | 16 | 协议 签订 签署 协议书 订立 协定 合同 达成 | 达成 协议 约定 谅解 生效 契约 签署 签 | 达成 签字 生效 条约 约定 谅解 |
| control t=0.7 unshifted q=0.3 | 17 | 协议 签订 签署 协定 协议书 订立 合同 条约 | 达成 协议 约定 生效 契约 签署 签 协议书 | 公约 签字 协议 签约 生效 协议书 |

## 银行

| Partition | Size | Centers | Fixed sample | Boundary |
|---|---:|---|---|---|
| archived recommendation | 30 | 存折 银行卡 支票 挂失 汇款 信用卡 身份证 取款 | 印章 支票 磁卡 汇款 存折 收据 卡 活期 | 活期 存款 外币 证件 银行 储蓄 |
| seeded baseline | 30 | 存折 银行卡 支票 挂失 汇款 信用卡 身份证 取款 | 印章 支票 磁卡 汇款 存折 收据 卡 活期 | 证件 钱包 银行 储蓄 钞票 活期 |
| control t=0.7 unshifted q=0.3 | 19 | 银行卡 存折 取款 信用卡 挂失 汇款 支票 取款机 | 支票 磁卡 汇款 存折 卡 活期 信用卡 储蓄 | 存款 外币 密码 卡 银行 卡片 |

## 软件

| Partition | Size | Centers | Fixed sample | Boundary |
|---|---:|---|---|---|
| archived recommendation | 7 | 人工智能 硬件 软件 计算机 智能 芯片 互联网 | 计算机 互联网 芯片 智能 软件 人工智能 硬件 | 软件 智能 互联网 芯片 人工智能 计算机 |
| seeded baseline | 12 | 硬盘 磁盘 浏览器 光盘 服务器 内存 杀毒 光碟 | 内存 服务器 硬盘 光碟 下载 杀毒 软件 光盘 | 下载 软件 防火墙 格式 浏览器 杀毒 |
| control t=0.7 unshifted q=0.3 | 11 | 服务器 软件 浏览器 杀毒 硬件 计算机 防火墙 内存 | 内存 计算机 服务器 芯片 黑客 杀毒 病毒 软件 | 黑客 病毒 防火墙 硬件 计算机 芯片 |

## 感冒

| Partition | Size | Centers | Fixed sample | Boundary |
|---|---:|---|---|---|
| archived recommendation | 46 | 咳嗽 头晕 呕吐 疼痛 痒 腹泻 疼 肿 | 胀 疼痛 咳嗽 喘 灼热 瘫 发作 上火 | 灼热 紊乱 喘 咽 渴 晕 |
| seeded baseline | 68 | 急性 病症 炎症 腹泻 症状 疾病 慢性 癌症 | 损伤 服用 疼痛 咳嗽 病情 患有 肿瘤 霍乱 | 疗效 抗生素 职业病 不适 昏迷 衰竭 |
| control t=0.7 unshifted q=0.3 | 65 | 急性 病症 炎症 腹泻 症状 慢性 疾病 发炎 | 疼痛 咳嗽 病情 患有 肿瘤 霍乱 精神病 癌 | 确诊 痛 昏迷 咳 职业病 痒 |

## 火车

| Partition | Size | Centers | Fixed sample | Boundary |
|---|---:|---|---|---|
| archived recommendation | 42 | 列车 火车 车票 公交车 车站 公共汽车 慢车 卧铺 | 线路 地铁 公交车 退票 车票 火车 公共汽车 搭乘 | 线路 座位 乘坐 搭乘 高铁 车厢 |
| seeded baseline | 40 | 列车 火车 车票 卧铺 慢车 车站 乘客 公共汽车 | 线路 地铁 退票 车票 火车 公共汽车 搭乘 航班 | 线路 登机 座位 长途 行李 高铁 |
| control t=0.7 unshifted q=0.3 | 39 | 列车 火车 车票 卧铺 车站 慢车 乘客 公共汽车 | 线路 地铁 退票 车票 火车 机票 公共汽车 搭乘 | 线路 登机 乘坐 搭乘 长途 行李 |

## 妈妈

| Partition | Size | Centers | Fixed sample | Boundary |
|---|---:|---|---|---|
| archived recommendation | 140 | 姑姑 舅舅 姐姐 妹妹 爸爸 嫂子 哥哥 奶奶 | 父女 她 老婆 家里 哥哥 姐妹 老乡 姑娘 | 亲属 老王 她们 战友 宝宝 朋友 |
| seeded baseline | 139 | 姑姑 舅舅 姐姐 妹妹 爸爸 嫂子 哥哥 奶奶 | 父女 她 老婆 家里 哥哥 姐妹 老乡 姑娘 | 老王 别人 她们 亲属 女生 朋友 |
| control t=0.7 unshifted q=0.3 | 134 | 姑姑 舅舅 姐姐 妹妹 嫂子 爸爸 哥哥 奶奶 | 父女 她 老婆 家里 哥哥 夫妇 姐妹 老乡 | 亲属 老王 朋友 男人 别人 贪玩儿 |

## 为什么

| Partition | Size | Centers | Fixed sample | Boundary |
|---|---:|---|---|---|
| archived recommendation | 27 | 怎么 怎样 什么 究竟 呢 哪里 哪儿 谁 | 请问 什么样 哪里 怎么 有没有 何处 谁 怎样 | 为何 为什么 有没有 多久 不知 什么样 |
| seeded baseline | 27 | 怎么 怎样 什么 究竟 呢 哪里 哪儿 谁 | 请问 什么样 哪里 怎么 有没有 何处 谁 怎样 | 不知 请问 什么样 有没有 吗 呢 |
| control t=0.7 unshifted q=0.3 | 27 | 怎么 怎样 什么 究竟 呢 哪里 哪儿 谁 | 请问 什么样 哪里 怎么 有没有 何处 谁 怎样 | 为何 为什么 不知 有没有 多久 请问 |

## 公斤

| Partition | Size | Centers | Fixed sample | Boundary |
|---|---:|---|---|---|
| archived recommendation | 19 | 公斤 千克 斤 吨 公顷 平方米 立方米 亩 | 桶 公斤 千克 毫升 吨 美元 平方 美金 | 立方米 平方 美元 克 人次 元 |
| seeded baseline | 19 | 公斤 千克 斤 立方米 公顷 平方米 亩 米 | 桶 尺 公斤 千克 毫升 吨 立方 毫米 | 尺 厘米 毫米 克 平方 毫升 |
| control t=0.7 unshifted q=0.3 | 20 | 公斤 千克 斤 公顷 平方米 吨 亩 磅 | 桶 尺 公斤 千克 毫升 吨 毫米 平方 | 立方米 平方 元 桶 毫米 克 |

## 花

| Partition | Size | Centers | Fixed sample | Boundary |
|---|---:|---|---|---|
| archived recommendation | 10 | 菊花 桃花 牡丹 梅花 玫瑰 花 鲜花 百合 | 桃花 梅花 花 玫瑰 菊花 芦花 绣 鲜花 | 绣 花 鲜花 芦花 牡丹 百合 |
| seeded baseline | 9 | 菊花 桃花 牡丹 玫瑰 梅花 花 百合 鲜花 | 桃花 梅花 花 玫瑰 菊花 芦花 鲜花 百合 | 花 芦花 鲜花 百合 玫瑰 桃花 |
| control t=0.7 unshifted q=0.3 | 13 | 荷花 菊花 牡丹 梅花 玫瑰 桃花 花瓣 花卉 | 花卉 桃花 花瓶 梅花 荷花 花 玫瑰 菊花 | 花卉 花瓶 芦花 花瓣 鲜花 花 |

## 行

| Partition | Size | Centers | Fixed sample | Boundary |
|---|---:|---|---|---|
| archived recommendation | 1 | 行 | 行 | 行 |
| seeded baseline | 1 | 行 | 行 | 行 |
| control t=0.7 unshifted q=0.3 | 1 | 行 | 行 | 行 |
